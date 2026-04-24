import json
import os

path = os.path.expanduser("~/Library/Application Support/tempest_ai/history.json")

with open(path, 'r') as f:
    history = json.load(f)

# Find the specific error message
new_history = []
skip_next = False
for i, msg in enumerate(history):
    if "could not be executed at this time" in msg.get('content', ''):
        # This is the rejection. We should remove it and the assistant message that preceded it.
        if len(new_history) > 0 and new_history[-1]['role'] == 'assistant':
            new_history.pop() # Remove the assistant message that was rejected
        continue # Skip the rejection itself
    new_history.append(msg)

with open(path, 'w') as f:
    json.dump(new_history, f, indent=2)

print(f"Pruned history. Removed rejection and preceding assistant message.")
