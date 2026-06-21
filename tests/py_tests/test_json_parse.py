import json
s = r'{"tool": "run_command", "arguments": {"command": "grep -rn \"Initiate Meltdown\" src/"}}'
try:
    print("Success:", json.loads(s))
except Exception as e:
    print("Error:", e)
