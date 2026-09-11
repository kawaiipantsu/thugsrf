# AI-assisted analysis

Set `ai_provider` (`openai`, `anthropic`, `local`) and `ai_model` in the Settings panel or configuration file. Model names are configurable because availability and capabilities vary. Local servers use an OpenAI-compatible `/v1/chat/completions` endpoint (Ollama, llama.cpp or similar); set `local_url` to its `/v1` base URL.

```sh
thugsrf config set ai_provider local
thugsrf config set ai_model YOUR_LOCAL_MODEL
thugsrf config set local_url http://127.0.0.1:11434/v1
thugsrf ai --input microphone.wav --format wav
thugsrf ai --image spectrum.png --question 'What modulation candidates fit this spectrum?'
```

Cloud credentials come from `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`; authenticated local servers may use `THUGSRF_LOCAL_API_KEY`. Credentials are never written to TOML or SQLite. AI requests only occur when you invoke `ai`. WAV and IQ inputs produce measured features locally; **raw audio is not uploaded or directly listened to by the model**. PNG/JPEG graphs are sent as image inputs and require a vision-capable model. Combine `--input` and `--image` to supply measurements and a graph together.

OpenAI uses the [Responses API](https://platform.openai.com/docs/api-reference/responses/create) with `store: false`; Anthropic uses Messages; local endpoints use Chat Completions. AI conclusions are stored separately as hypotheses. TLS is required except for loopback local servers. There is no automatic web browsing or autonomous RF action.


Select a model supported by your provider and, for images, one with vision support. Model names are deliberately not hard-coded. Review the output alongside measured spectra and decoder results. The application does not infer transmitter identity reliably from an image alone. Provider usage may incur charges. See [[Command-Reference]] for all `ai` options.
