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
- Track uploaded logs with visual highlighting

### 🔔 **Discord Integration**
- Send reports directly to Discord channels via webhooks
- Save and manage multiple webhooks
- Customize report names with automatic date insertion
- Batch send all reports (main + legacy) at once

### 🧹 **Disk Space Management**
- **Automatic Cleanup**: Optionally clean old logs once per session on plugin load
- **Manual Cleanup**: On-demand cleanup with customizable age threshold
- **Safe Operation**: Files moved to Recycle Bin (restorable if needed)
- **Configurable**: Set custom age thresholds (default: 30 days)

### 🔐 **Token Management**
- Generate history tokens directly from the addon
- Save multiple tokens for different accounts
- Token validation before saving
- Quick token switching
- Sync log directory automatically from ArcDPS config

### 📊 **Report History**
- Track all your uploaded reports
- Main and legacy reports grouped by session
- Quick access to previous parses
- Copy links or open in browser
- View all reports for your token on the website

### ⚙️ **Customization**
- Configurable API endpoint
- Guild name customization for reports
- Optional legacy parser support (not recommended)
- Toggle between formatted/raw timestamps
- Keybind support (Default: `Ctrl+Shift+W`)
- Quick access icon integration
- Optional mouse lock feature

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
   - (Optional) Set your guild name in Settings

2. **Upload Logs**:
   - Open the addon window
   - Select your desired time filter
   - Check the logs you want to upload
   - Click "Upload Selected"
   - Start processing when uploads complete

3. **Share to Discord**:
   - After processing completes, click "Send to Discord"
   - Select a saved webhook or enter a new one
   - Customize the report name
   - Send to your Discord channel

4. **Manage Disk Space**:
   - Go to Settings → Cleanup tab
   - Enable automatic cleanup or run manual cleanup
   - Set your preferred age threshold for old logs

## Configuration

Settings are stored in: `Guild Wars 2/addons/wvw-insights/settings.json`

### Settings Options
- **General**: Log directory, API endpoint, display options, guild name, legacy parser
- **Token Manager**: Save and switch between multiple tokens with validation
- **Webhooks**: Manage Discord webhooks for easy report sharing
- **Cleanup**: Configure automatic/manual cleanup settings
- **History**: View and manage your report history
- **QoL**: Optional mouse lock feature

## Development

Built with Rust using the [nexus-rs](https://github.com/Zerthox/nexus-rs) framework.
