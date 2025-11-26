# Ripley REST API

REST API for remote control of the Ripley disc ripper with real-time WebSocket updates.

## Starting the Server

```bash
# Default: http://127.0.0.1:3000
ripley serve

# Custom port
ripley serve --port 8080

# Listen on all interfaces (for remote access)
ripley serve --host 0.0.0.0 --port 3000
```

## API Endpoints

### Health Check
```bash
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

### Get Rip Status
```bash
GET /status
```

**Response:**
```json
{
  "is_ripping": false,
  "current_disc": null,
  "current_title": null,
  "progress": 0.0,
  "logs": []
}
```

### Get Configuration
```bash
GET /config
```

**Response:**
```json
{
  "openai_api_key": null,
  "tmdb_api_key": "fef1285fb85a74350b3292b5fac37fce",
  "notifications": {
    "enabled": true,
    "topic": "ripley"
  },
  "rsync": {
    "enabled": true,
    "destination": "/Volumes/Media/Rips"
  },
  "speech_match": {
    "enabled": true,
    "audio_duration": 180,
    "whisper_model": "whisper-1",
    "use_openai_api": true
  },
  "filebot": {
    "skip_by_default": false,
    "database": "TheTVDB",
    "order": "Airdate",
    "use_for_music": true
  }
}
```

### Update Configuration
```bash
POST /config
Content-Type: application/json

{
  "openai_api_key": "sk-...",
  "tmdb_api_key": "fef1285fb85a74350b3292b5fac37fce",
  "notifications": {
    "enabled": true,
    "topic": "ripley"
  },
  "rsync": {
    "enabled": true,
    "destination": "/Volumes/Media/Rips"
  },
  "speech_match": {
    "enabled": true,
    "audio_duration": 180,
    "whisper_model": "whisper-1",
    "use_openai_api": true
  },
  "filebot": {
    "skip_by_default": false,
    "database": "TheTVDB",
    "order": "Airdate",
    "use_for_music": true
  }
}
```

**Response:** Returns the updated configuration.

### List Optical Drives
```bash
GET /drives
```

**Response:**
```json
[
  {
    "device": "/dev/disk2",
    "name": "Drive /dev/disk2",
    "has_audio_cd": false,
    "media_type": "DVD"
  }
]
```

### Start Ripping
```bash
POST /rip/start
Content-Type: application/json

{
  "output_path": "/Users/johnny/Desktop/Rips/Video",
  "title": "Star Trek TNG",
  "skip_metadata": false,
  "skip_filebot": false
}
```

**Response:**
```json
{
  "status": "started"
}
```

### Stop Ripping
```bash
POST /rip/stop
```

**Response:**
```json
{
  "status": "stopped"
}
```

### Rename Existing Files
```bash
POST /rename
Content-Type: application/json

{
  "directory": "/path/to/video/files",
  "title": "The Office",
  "skip_speech": false,
  "skip_filebot": false
}
```

**Response:**
```json
{
  "status": "started"
}
```

## WebSocket API

Connect to `ws://localhost:3000/ws` for real-time updates.

### Event Types

#### RipStarted
```json
{
  "type": "RipStarted",
  "data": {
    "disc": "Star Trek TNG Season 1"
  }
}
```

#### RipProgress
```json
{
  "type": "RipProgress",
  "data": {
    "progress": 0.42,
    "message": "Processing title 3 of 26..."
  }
}
```

#### RipCompleted
```json
{
  "type": "RipCompleted",
  "data": {
    "disc": "Star Trek TNG Season 1"
  }
}
```

#### RipError
```json
{
  "type": "RipError",
  "data": {
    "error": "Failed to detect disc"
  }
}
```

#### Log
```json
{
  "type": "Log",
  "data": {
    "message": "Extracting subtitles from MKV..."
  }
}
```

#### StatusUpdate
```json
{
  "type": "StatusUpdate",
  "data": {
    "status": {
      "is_ripping": true,
      "current_disc": "Star Trek TNG Season 1",
      "current_title": "Encounter at Farpoint",
      "progress": 0.42,
      "logs": [
        "Starting rip...",
        "Processing MKV..."
      ]
    }
  }
}
```

## Example: JavaScript Client

```javascript
// Connect to WebSocket
const ws = new WebSocket('ws://localhost:3000/ws');

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  switch (message.type) {
    case 'RipProgress':
      console.log(`Progress: ${message.data.progress * 100}%`);
      console.log(`Status: ${message.data.message}`);
      break;
      
    case 'RipCompleted':
      console.log(`Completed: ${message.data.disc}`);
      break;
      
    case 'Log':
      console.log(message.data.message);
      break;
  }
};

// Start ripping
async function startRip() {
  const response = await fetch('http://localhost:3000/rip/start', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      output_path: '/Users/johnny/Desktop/Rips/Video',
      title: 'Star Trek TNG',
      skip_metadata: false,
      skip_filebot: false
    })
  });
  
  const result = await response.json();
  console.log(result);
}

// Get current status
async function getStatus() {
  const response = await fetch('http://localhost:3000/status');
  const status = await response.json();
  console.log(status);
}
```

## Example: curl Commands

```bash
# Check health
curl http://localhost:3000/health

# Get current status
curl http://localhost:3000/status

# List drives
curl http://localhost:3000/drives

# Start ripping
curl -X POST http://localhost:3000/rip/start \
  -H "Content-Type: application/json" \
  -d '{
    "output_path": "/Users/johnny/Desktop/Rips/Video",
    "title": "Star Trek TNG",
    "skip_metadata": false,
    "skip_filebot": false
  }'

# Stop ripping
curl -X POST http://localhost:3000/rip/stop

# Rename files
curl -X POST http://localhost:3000/rename \
  -H "Content-Type: application/json" \
  -d '{
    "directory": "/Users/johnny/Desktop/Rips/Video",
    "title": "The Office",
    "skip_speech": false,
    "skip_filebot": false
  }'
```

## CORS

The API includes permissive CORS headers, allowing requests from any origin. This is suitable for development but should be restricted in production.

## Error Responses

All endpoints may return error responses in this format:

```json
{
  "error": "Error message description"
}
```

HTTP status codes:
- `200 OK` - Success
- `500 Internal Server Error` - Server error

## Testing

Run the API integration tests:

```bash
cargo test --test api_test
```

All 10 API tests should pass:
- `test_api_state_creation`
- `test_rip_status_default`
- `test_api_event_serialization`
- `test_broadcast_channel`
- `test_config_read_write`
- `test_rip_status_updates`
- `test_status_update_event`
- `test_multiple_log_events`
- `test_rip_status_serialization`
- `test_config_clone`

## Next Steps

To build a web UI, you can:

1. Create a React/Vue/Svelte app that connects to this API
2. Use the WebSocket for real-time status updates
3. Build a dashboard showing:
   - Current rip status and progress
   - Detected drives and media
   - Recent logs
   - Configuration editor
   
The API provides all the functionality needed for a full-featured web interface.
