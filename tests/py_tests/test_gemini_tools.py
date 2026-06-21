import os
import requests
import json

api_key = os.environ.get("GEMINI_API_KEY")
url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"

headers = {
    "Authorization": f"Bearer {api_key}",
    "Content-Type": "application/json"
}

data = {
    "model": "gemini-3.1-pro-preview-customtools",
    "messages": [
        {"role": "user", "content": "What is the current weather in Paris? Please use the get_weather tool immediately."}
    ],
    "tools": [{
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get current weather in a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }
        }
    }],
    "stream": True
}

response = requests.post(url, headers=headers, json=data, stream=True)
for line in response.iter_lines():
    if line:
        print(line.decode('utf-8'))
