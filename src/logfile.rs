use std::path::PathBuf;

// State change constants
const CBTS_MAPID: u8 = 25;
const CBTS_POINTOFVIEW: u8 = 13;
const MARKER_STATECHANGE: u8 = 37;
const COMMANDER_MARKER_VALUE: u8 = 1;

// Profession and specialization names to filter out from commander detection
const PROF_OR_SPEC_NAMES: &[&str] = &[
    "Guardian", "Warrior", "Revenant", "Engineer", "Ranger", "Thief", "Elementalist", "Mesmer", "Necromancer",
    "Dragonhunter", "Firebrand", "Willbender",
    "Berserker", "Spellbreaker", "Bladesworn",
    "Herald", "Renegade", "Vindicator",
    "Scrapper", "Holosmith", "Mechanist",
    "Druid", "Soulbeast", "Untamed",
    "Daredevil", "Deadeye", "Specter",
    "Tempest", "Weaver", "Catalyst",
    "Chronomancer", "Mirage", "Virtuoso",
    "Reaper", "Scourge", "Harbinger",
];

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
            38 => MapType::EternalBattlegrounds,
            95 => MapType::GreenAlpineBorderlands,
            96 => MapType::BlueAlpineBorderlands,
            1099 => MapType::RedDesertBorderlands,
            968 => MapType::EdgeOfTheMists,
            899 => MapType::ObsidianSanctum,
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
pub struct EVTCAgent {
    pub addr: u64,
    pub character: String,
    pub account: String,
}

impl EVTCAgent {
    fn from_bytes(data: &[u8], offset: usize) -> Option<Self> {
        if offset + 96 > data.len() {
            return None;
        }

        let addr = u64::from_le_bytes([
            data[offset], data[offset+1], data[offset+2], data[offset+3],
            data[offset+4], data[offset+5], data[offset+6], data[offset+7]
        ]);

        // Name is at offset 28, 64 bytes
        let name_bytes = &data[offset + 28..offset + 92];
        
        // Split by null bytes and decode
        let parts: Vec<String> = name_bytes
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut character = String::new();
        let mut account = String::new();

        if let Some(first) = parts.first() {
            if first.contains(':') {
                let segs: Vec<&str> = first.split(':').collect();
                if segs.len() >= 2 {
                    character = segs[0].trim().to_string();
                    account = segs[1].trim().to_string();
                } else {
                    character = first.trim().to_string();
                }
            } else {
                character = first.trim().to_string();
                if parts.len() > 1 {
                    account = parts[1].trim_start_matches(':').trim().to_string();
                }
            }
        }

        Some(EVTCAgent { addr, character, account })
    }

    fn is_player(&self) -> bool {
        // Account names should match pattern: Name.XXXX
        if self.account.is_empty() {
            return false;
        }
        
        // Check if account ends with .XXXX where X is a digit
        if let Some(dot_pos) = self.account.rfind('.') {
            let suffix = &self.account[dot_pos + 1..];
            suffix.len() == 4 && suffix.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    }

    fn is_valid_commander_candidate(&self) -> bool {
        self.is_player() && !PROF_OR_SPEC_NAMES.contains(&self.character.as_str())
    }

    pub fn display_name(&self) -> String {
        if !self.character.is_empty() {
            self.character.clone()
        } else {
            format!("0x{:x}", self.addr)
        }
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
    pub recorder: Option<String>,
    pub commander: Option<String>,
}


/// Parse agents from EVTC data
fn parse_agents(data: &[u8]) -> Option<(Vec<EVTCAgent>, usize)> {
    if data.len() < 16 {
        return None;
    }

    let mut pos = 16; // Skip header

    // Read agent count
    if pos + 4 > data.len() {
        return None;
    }
    let agent_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;

    // Parse agents
    let mut agents = Vec::new();
    for _ in 0..agent_count {
        if let Some(agent) = EVTCAgent::from_bytes(data, pos) {
            agents.push(agent);
        }
        pos += 96;
    }

    Some((agents, pos))
}

/// Extract recorder, commander, and map info from EVTC bytes
fn read_evtc_info_from_bytes(data: &[u8]) -> Option<(u16, MapType, Option<String>, Option<String>)> {
    if data.len() < 16 {
        return None;
    }
    
    let revision = data[12];
    
    // Parse agents
    let (agents, mut pos) = parse_agents(data)?;
    
    // Skip skill count
    if pos + 4 > data.len() {
        return None;
    }
    let skill_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;
    
    // Skip skills
    let skill_data_size = skill_count * 68;
    if pos + skill_data_size > data.len() {
        return None;
    }
    pos += skill_data_size;
    
    // State change offset depends on revision
    let state_change_offset = if revision == 1 { 56 } else { 59 };
    
    let mut map_id = 0u16;
    let mut recorder_addr = None;
    let mut commander_counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    
    // Scan combat items (limit to reasonable amount)
    let max_items = ((data.len() - pos) / 64).min(10000);
    
    for _ in 0..max_items {
        if pos + 64 > data.len() {
            break;
        }
        
        let state_change = data[pos + state_change_offset];
        
        // Check for map ID
        if state_change == CBTS_MAPID && map_id == 0 {
            map_id = u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
        }
        
        // Check for point of view (recorder)
        if state_change == CBTS_POINTOFVIEW && recorder_addr.is_none() {
            let src = u64::from_le_bytes([
                data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11],
                data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]
            ]);
            recorder_addr = Some(src);
        }
        
        // Check for commander tag
        if state_change == MARKER_STATECHANGE && data[pos + 49] == COMMANDER_MARKER_VALUE {
            let src = u64::from_le_bytes([
                data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11],
                data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]
            ]);
            *commander_counts.entry(src).or_insert(0) += 1;
        }
        
        pos += 64;
    }
    
    let map_type = MapType::from_map_id(map_id);
    
    // Find recorder name
    let recorder = recorder_addr.and_then(|addr| {
        agents.iter()
            .find(|a| a.addr == addr)
            .map(|a| a.display_name())
    });
    
    // Find most common commander
    let commander = commander_counts.into_iter()
        .max_by_key(|(_, count)| *count)
        .and_then(|(addr, _)| {
            agents.iter()
                .find(|a| a.addr == addr && a.is_valid_commander_candidate())
                .map(|a| a.display_name())
        });
    
    Some((map_id, map_type, recorder, commander))
}

/// Efficiently reads info from EVTC file
fn read_evtc_info(file_path: &std::path::Path) -> Option<(u16, MapType, Option<String>, Option<String>)> {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(file_path).ok()?;
    
    // Read first few bytes to check file type
    let mut header_buffer = [0u8; 4];
    file.read_exact(&mut header_buffer).ok()?;
    
    // Check if it's a ZIP file
    if header_buffer[0] == 0x50 && header_buffer[1] == 0x4B {
        // For ZIP files, we need to decompress
        drop(file);
        
        let mut file = File::open(file_path).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        
        if buffer.len() < 30 {
            return None;
        }
        
        let mut pos = 30;
        let file_name_length = u16::from_le_bytes([buffer[26], buffer[27]]) as usize;
        pos += file_name_length;
        let extra_field_length = u16::from_le_bytes([buffer[28], buffer[29]]) as usize;
        pos += extra_field_length;
        
        if pos >= buffer.len() {
            return None;
        }
        
        use flate2::read::DeflateDecoder;
        let compressed_data = &buffer[pos..];
        let mut decoder = DeflateDecoder::new(compressed_data);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data).ok()?;
        
        return read_evtc_info_from_bytes(&decompressed_data);
    }
    
    // Uncompressed EVTC
    drop(file);
    let mut file = File::open(file_path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    
    read_evtc_info_from_bytes(&data)
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

        // Read all info efficiently
        let (_map_id, map_type, recorder, commander) = read_evtc_info(&path).unwrap_or_else(|| {
            log::warn!("Failed to read EVTC info from: {:?}", path);
            (0, MapType::Unknown, None, None)
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
            recorder,
            commander,
        })
    }
}