# Spotify Web API — LLM Tool Proposal

## Auth Layer

OAuth 2.0 Authorization Code with PKCE (no client secret needed for desktop app).
- Access tokens: 1 hour lifetime
- Refresh tokens: 6 month lifetime (June 2026 policy)
- Refreshing does not extend lifetime; re-authorization required after 6 months
- Playback endpoints require Spotify Premium

FastMD would need: a local token store (`~/.fastmd/spotify_token.json`), inline browser OAuth flow, and automatic token refresh with `invalid_grant` handling.

## Suggested Tools

### 1. Discovery & Search

| Tool | Endpoint | Purpose |
|---|---|---|
| `spotify_search` | `GET /search?q=&type=&limit=` | Search tracks, albums, artists, playlists, shows (max 10 results per 2026 limit) |
| `spotify_get_track` | `GET /tracks/{id}` | Full track metadata |
| `spotify_get_album` | `GET /albums/{id}` | Album details + track listing |
| `spotify_get_artist` | `GET /artists/{id}` | Artist metadata + genres |
| `spotify_get_artist_albums` | `GET /artists/{id}/albums` | Discography lookup |

### 2. Playlist Management

| Tool | Endpoint | Purpose |
|---|---|---|
| `spotify_list_my_playlists` | `GET /me/playlists` | Enumerate user's playlists |
| `spotify_get_playlist` | `GET /playlists/{id}` | Playlist details + items |
| `spotify_create_playlist` | `POST /me/playlists` | Create new playlist |
| `spotify_add_to_playlist` | `POST /playlists/{id}/items` | Add tracks |
| `spotify_remove_from_playlist` | `DELETE /playlists/{id}/items` | Remove tracks |
| `spotify_update_playlist_items` | `PUT /playlists/{id}/items` | Reorder/replace items |

### 3. Library (consolidated in 2026 to generic `/me/library`)

| Tool | Endpoint | Purpose |
|---|---|---|
| `spotify_save_to_library` | `PUT /me/library` | Save items by Spotify URI |
| `spotify_remove_from_library` | `DELETE /me/library` | Remove items |
| `spotify_check_library` | `GET /me/library/contains` | Check if items are saved |

### 4. Playback Control (Premium only)

| Tool | Endpoint | Purpose |
|---|---|---|
| `spotify_get_playback_state` | `GET /me/player` | Current playback state |
| `spotify_get_devices` | `GET /me/player/devices` | Available devices |
| `spotify_play` | `PUT /me/player/play` | Start/resume (context URI or track URIs) |
| `spotify_pause` | `PUT /me/player/pause` | Pause |
| `spotify_next` | `POST /me/player/next` | Skip |
| `spotify_previous` | `POST /me/player/previous` | Previous |
| `spotify_seek` | `PUT /me/player/seek` | Seek to position (ms) |
| `spotify_set_volume` | `PUT /me/player/volume` | Volume 0-100 |
| `spotify_set_repeat` | `PUT /me/player/repeat` | Repeat mode |
| `spotify_set_shuffle` | `PUT /me/player/shuffle` | Toggle shuffle |
| `spotify_add_to_queue` | `POST /me/player/queue` | Add to queue |
| `spotify_get_queue` | `GET /me/player/queue` | View queue |
| `spotify_transfer_playback` | `PUT /me/player` | Transfer to device |

### 5. User Context

| Tool | Endpoint | Purpose |
|---|---|---|
| `spotify_get_my_profile` | `GET /me` | Current user info |
| `spotify_get_recently_played` | `GET /me/player/recently-played` | Recent tracks |
| `spotify_get_my_top_items` | `GET /me/top/{type}` | Top artists/tracks (short/medium/long term) |

## Simplification Options

**Full (25+ tools):** All of the above. Maximum agent capability.

**Lean (~10 tools):** Collapse playback into `spotify_control_playback` (play/pause/next/previous as action parameter) + `spotify_get_state` (playback + device + queue).

**Minimal (5 tools):** `spotify_search`, `spotify_get_details` (any item by URI), `spotify_manage_playlist` (create/add/remove), `spotify_playback` (play/pause/skip/queue), `spotify_user_data` (profile + top + recent).
