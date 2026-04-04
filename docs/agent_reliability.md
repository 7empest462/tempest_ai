# Agent Reliability & Loop Recovery

This document explains the autonomous safety systems used to keep Tempest AI productive, especially when using smaller models like Mistral 7B.

## 🛠️ Loop Detection
The agent maintains a cache of its 5 most recent tool calls. Before executing any command, it checks if it is about to repeat a command with identical arguments. 

If a repeat is detected:
- The execution is BLOCKED.
- You will see: `❌ Loop Detected. Intercepting duplicate tool sequence...`

## 🧠 Autonomous Recovery
In earlier versions, a loop detection would terminate the task. Now, the agent attempts to "Self-Correct" autonomously:
1. **Instruction Injection**: The system injects a high-priority instruction: *"STOP: You just attempted to execute the exact same tool as a previous turn... PIVOT to a new strategy."*
2. **Immediate Re-Trigger**: Instead of dropping back to the prompt, the agent immediately initiates a new inference cycle. This forces the AI to acknowledge its repetition and try a different approach.

## 📍 Spatial Orientation (Project Atlas)
To minimize repetitive orientation loops, the standard system prompt instructions for `project_atlas` have been softened from mandatory requirements to situational guidance:
- **Condition-based**: The AI is told to use it *only if* it doesn't yet understand the layout.
- **History Checking**: The AI is explicitly forbidden from repeating the command if the result is already present in its recent conversation history.
