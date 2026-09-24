# WhipScribe Recorder

A macOS desktop app (Tauri + Rust + Svelte) that knows your calendar and records your meetings without a bot joining the call.
Recordings are transcribed through the WhipScribe API and organised in your WhipScribe library through the MCP server.

## The four steps

1. **Calendar**: Connect Google Calendar and see your next 7 days of meetings, with Join links and a Record button on meetings that are live or starting soon.
2. **Recording**: Records your microphone and the call audio together, with pause and level meters. The file is saved every second and recovered if the app closes mid-call.
3. **Transcript**: Stopping uploads the recording to the WhipScribe API, shows progress, and opens the transcript with timestamps (and speaker labels when the API returns them).
4. **Library**: Sign in to WhipScribe to create folders and file recordings into them over MCP; rename, delete and search every transcript.

## Extras

- **Live transcript** while recording: rough text every few seconds, then the full transcript when you stop.
- **Search** across all transcripts with ⌘F, jumping to the exact moment.
- **Home** with upcoming meetings and recent recordings; a sidebar with folders; the recording dock at the bottom so the page doesn't move.
- **Offline**: transcripts are saved locally and open without internet.
- **Secrets in the Keychain**: Google, the WhipScribe API key and the sign-in token never touch a file.

## Run it

You need macOS 14.2+, [Rust](https://rustup.rs), [Bun](https://bun.sh) and Xcode Command Line Tools.
Create a Google Cloud project with the Calendar API enabled and an OAuth client of type **Desktop app**, and add yourself as a test user.

```sh
cd apps/amritesh/desktop
cp .env.example .env   # add GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET
bun install
bun run tauri dev
```

The WhipScribe API key (whipscribe.com → Account → API key) is pasted in the app the first time you transcribe.
