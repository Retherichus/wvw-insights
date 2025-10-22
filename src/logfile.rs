use std::path::PathBuf;

// The correct state change value for map ID
const CBTS_MAPID: u8 = 25;

#[derive(Debug, Clone, PartialEq)]
pub enum MapType {
    EternalBattlegrounds,
    GreenAlpineBorderlands,
    BlueAlpineBorderlands,
    RedDesertBorderlands,
    EdgeOfTheMists,
    ObsidianSanctum,
    PvE,
    Unknown,
}

impl MapType {
    pub fn from_map_id(map_id: u16) -> Self {
        match map_id {
            // Eternal Battlegrounds
            38 => MapType::EternalBattlegrounds,
            
            // Alpine Borderlands
            95 => MapType::GreenAlpineBorderlands,
            96 => MapType::BlueAlpineBorderlands,
            
            // Desert Borderlands  
            1099 => MapType::RedDesertBorderlands,
            
            // Edge of the Mists
            968 => MapType::EdgeOfTheMists,
            
            // Obsidian Sanctum
            899 => MapType::ObsidianSanctum,
            
            // Everything else is PvE or unknown
            _ => {
                if map_id > 0 { MapType::PvE } else { MapType::Unknown }
            }
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            MapType::EternalBattlegrounds => "EBG",
            MapType::GreenAlpineBorderlands => "GBL",
            MapType::BlueAlpineBorderlands => "BBL",
            MapType::RedDesertBorderlands => "RBL",
            MapType::EdgeOfTheMists => "EotM", 
            MapType::ObsidianSanctum => "OS",
            MapType::PvE => "PvE",
            MapType::Unknown => "Unknown",
        }
    }
    
    pub fn is_wvw(&self) -> bool {
        !matches!(self, MapType::PvE | MapType::Unknown)
    }
}

#[derive(Debug, Clone)]
pub struct LogFile {
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: u64,
    pub selected: bool,
    pub uploaded: bool,
    pub status: String,
    pub map_type: MapType,
}

// Removed is_valid_wvw_map_id - no longer needed since we check map_type.is_wvw() instead

/// Reads map ID from ZIP-compressed EVTC file with decompression
fn read_evtc_map_id_from_zip_decompressed(file_path: &std::path::Path) -> Option<(u16, MapType)> {
    use std::fs::File;
    use std::io::Read;
    use flate2::read::DeflateDecoder;
    
    // Read the entire file
    let mut file = File::open(file_path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    
    // Parse ZIP structure manually
    if buffer.len() < 30 {
        return None;
    }
    
    // Skip ZIP local file header (30 bytes minimum)
    let mut pos = 30;
    
    // Skip file name
    if buffer.len() < 28 {
        return None;
    }
    let file_name_length = u16::from_le_bytes([buffer[26], buffer[27]]) as usize;
    pos += file_name_length;
    
    // Skip extra field length
    let extra_field_length = u16::from_le_bytes([buffer[28], buffer[29]]) as usize;
    pos += extra_field_length;
    
    // Now we're at the compressed data
    if pos >= buffer.len() {
        return None;
    }
    
    let compressed_data = &buffer[pos..];
    
    // Decompress the data
    let mut decoder = DeflateDecoder::new(compressed_data);
    let mut decompressed_data = Vec::new();
    decoder.read_to_end(&mut decompressed_data).ok()?;
    
    // Now parse the decompressed EVTC data
    read_evtc_map_id_from_bytes(&decompressed_data)
}

/// Reads map ID from uncompressed EVTC data
fn read_evtc_map_id_from_bytes(data: &[u8]) -> Option<(u16, MapType)> {
    if data.len() < 16 {
        return None;
    }
    
    // Parse EVTC header
    // Bytes 0-11: build version string
    // Byte 12: revision
    // Bytes 13-14: boss species ID (NOT map ID!)
    // Byte 15: unused
    
    let revision = data[12];
    
    let mut pos = 16; // Start after header
    
    // Read agent count (4 bytes)
    if pos + 4 > data.len() {
        return None;
    }
    let agent_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;
    
    // Skip agents (96 bytes each)
    let agent_data_size = agent_count * 96;
    if pos + agent_data_size > data.len() {
        return None;
    }
    pos += agent_data_size;
    
    // Read skill count (4 bytes)
    if pos + 4 > data.len() {
        return None;
    }
    let skill_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;
    
    // Skip skills (68 bytes each)
    let skill_data_size = skill_count * 68;
    if pos + skill_data_size > data.len() {
        return None;
    }
    pos += skill_data_size;
    
    // Now we're at combat items (64 bytes each)
    // State change offset depends on revision
    let state_change_offset = if revision == 1 { 56 } else { 59 };
    
    // Scan up to 100 combat items looking for CBTS_MAPID
    let max_items = ((data.len() - pos) / 64).min(100);
    
    for _ in 0..max_items {
        if pos + 64 > data.len() {
            break;
        }
        
        let state_change = data[pos + state_change_offset];
        
        // CBTS_MAPID = 25
        if state_change == CBTS_MAPID {
            // Map ID is in src_agent (bytes 8-9 for the u16 value)
            let map_id = u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
            let map_type = MapType::from_map_id(map_id);
            return Some((map_id, map_type));
        }
        
        pos += 64;
    }
    
    None
}

/// Efficiently reads just the map ID from EVTC file
fn read_evtc_map_id(file_path: &std::path::Path) -> Option<(u16, MapType)> {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(file_path).ok()?;
    
    // Read the first few bytes to check file type
    let mut header_buffer = [0u8; 4];
    file.read_exact(&mut header_buffer).ok()?;
    
    // Check if it's a ZIP file (PK header: 0x50 0x4B)
    if header_buffer[0] == 0x50 && header_buffer[1] == 0x4B {
        return read_evtc_map_id_from_zip_decompressed(file_path);
    }
    
    // Not a ZIP file, read the entire uncompressed EVTC
    drop(file); // Close the file handle
    
    let mut file = File::open(file_path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    
    read_evtc_map_id_from_bytes(&data)
}

impl LogFile {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let modified = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Read map info efficiently
        let (_map_id, map_type) = read_evtc_map_id(&path).unwrap_or_else(|| {
            log::warn!("Failed to read map ID from: {:?}", path);
            (0, MapType::Unknown)
        });

        Ok(Self {
            path,
            filename,
            size: metadata.len(),
            modified,
            selected: false,
            uploaded: false,
            status: "Ready".to_string(),
            map_type,
        })
    }
}