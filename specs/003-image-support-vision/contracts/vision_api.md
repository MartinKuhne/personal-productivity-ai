# Contract: Vision API Payload

This project acts as an API client to OpenAI-compatible vision APIs (e.g., OpenRouter). The outgoing payload must match the following schema:

```json
{
  "model": "gpt-4o-mini",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "Describe this image in detailed Markdown. Include text, objects, scenes, charts, diagrams, and UI elements."
        },
        {
          "type": "image_url",
          "image_url": {
            "url": "data:image/jpeg;base64,<BASE64_STRING>"
          }
        }
      ]
    }
  ]
}
```

- Base64 encoding must omit newlines.
- MIME type must correctly reflect the file extension (e.g., `image/jpeg`, `image/png`, `image/webp`).
