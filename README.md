# WvW Insights

A [Raidcore Nexus](https://github.com/RaidcoreGG/Nexus) addon for Guild Wars 2 that streamlines uploading and analyzing your WvW combat logs.

## Features

### 📤 **Streamlined Log Uploads**
- Select and upload multiple ArcDPS combat logs directly from the game
- Automatic session management with batch processing
- Real-time upload progress tracking
- Integration with [WvW Insights Parser](https://parser.rethl.net)

### 🔍 **Smart Log Management**
- Multiple time filters (session, 24h, 48h, 72h, all logs)
- Recursive subdirectory scanning
- Automatic refresh in session mode (every 20 seconds)
- Formatted timestamps for easy log identification

### 🧹 **Disk Space Management**
- **Automatic Cleanup**: Optionally clean old logs once per session on plugin load
- **Manual Cleanup**: On-demand cleanup with customizable age threshold
- **Safe Operation**: Files moved to Recycle Bin (restorable if needed)
- **Configurable**: Set custom age thresholds (default: 30 days)

### 🔐 **Token Management**
- Generate history tokens directly from the addon
- Save multiple tokens for different accounts
- Quick token switching
- Sync log directory automatically from ArcDPS config

### 📊 **Report History**
- Track all your uploaded reports
- Quick access to previous parses
- Copy links or open in browser
- View all reports for your token on the website

### ⚙️ **Customization**
- Configurable API endpoint
- Toggle between formatted/raw timestamps
- Keybind support (Default: `Ctrl+Shift+W`)
- Quick access icon integration

## Installation

1. Install [Raidcore Nexus](https://github.com/RaidcoreGG/Nexus)
2. Install [ArcDPS](https://www.deltaconnected.com/arcdps/)
3. Download the latest release from the [Releases](https://github.com/Retherichus/wvw-insights/releases) page
4. Place `wvw_insights.dll` in your `Guild Wars 2/addons/` folder
5. Launch Guild Wars 2

The addon will auto-update through Nexus.

## Usage

1. **First Time Setup**:
   - Click the WvW Insights icon or press `Ctrl+Shift+W`
   - Generate or enter your history token
   - Configure your log directory (or sync from ArcDPS)

2. **Upload Logs**:
   - Open the addon window
   - Select your desired time filter
   - Check the logs you want to upload
   - Click "Upload Selected"
   - Start processing when uploads complete

3. **Manage Disk Space**:
   - Go to Settings → Cleanup tab
   - Enable automatic cleanup or run manual cleanup
   - Set your preferred age threshold for old logs

## Configuration

Settings are stored in: `Guild Wars 2/addons/wvw-insights/settings.json`

### Settings Options
- **Log Directory**: Path to your ArcDPS combat logs
- **API Endpoint**: Parser API URL (default: https://parser.rethl.net/api.php)
- **Display Options**: Toggle formatted timestamps
- **Token Manager**: Save and switch between multiple tokens
- **Cleanup**: Configure automatic/manual cleanup settings

## Development

Built with Rust using the [nexus-rs](https://github.com/Zerthox/nexus-rs) framework.
