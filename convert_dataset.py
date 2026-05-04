import json
import os
import random

input_file = "/Volumes/Corsair_Lab/Home/Projects/tempest_ai/tempest_dataset.jsonl"
output_dir = "/Volumes/Corsair_Lab/Home/Projects/tempest_ai/tempest_train_v9"

os.makedirs(output_dir, exist_ok=True)

with open(input_file, "r") as f:
    lines = [line.strip() for line in f if line.strip()]

formatted_lines = []
for line in lines:
    data = json.loads(line)
    convs = data["conversations"]
    text = ""
    # Support multi-turn if present, but standard is 2 or 4 or 6...
    for i in range(0, len(convs)):
        role = convs[i]["from"]
        val = convs[i]["value"]
        if role == "human":
            text += f"<｜User｜>{val}"
        elif role == "gpt":
            text += f"<｜Assistant｜>{val}"
        elif role == "tool":
            # For DeepSeek R1 distill, we treat tool results as a User-like injection 
            # Or we can just use the <｜User｜> tag. 
            # In our train.jsonl (v7) we saw it was merged.
            text += f"<｜User｜>{val}"
    
    formatted_lines.append(json.dumps({"text": text}))

# Shuffle for better distribution
random.seed(42)
random.shuffle(formatted_lines)

split_idx = int(len(formatted_lines) * 0.9)
train_data = formatted_lines[:split_idx]
valid_data = formatted_lines[split_idx:]

with open(os.path.join(output_dir, "train.jsonl"), "w") as f:
    f.write("\n".join(train_data))

with open(os.path.join(output_dir, "valid.jsonl"), "w") as f:
    f.write("\n".join(valid_data))

print(f"Dataset v8 prepared: {len(train_data)} train, {len(valid_data)} valid.")
