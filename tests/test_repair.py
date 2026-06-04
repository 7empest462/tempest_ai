import re
import json

def repair_json_str(s: str) -> str:
    # Find all key-value starts where the value begins with a quote.
    # Pattern matches `"key": "`
    key_start_re = re.compile(r'"[a-zA-Z0-9_-]+"\s*:\s*"')
    matches = list(key_start_re.finditer(s))
    matches.reverse()

    result = s
    for m in matches:
        start_idx = m.start()
        end_idx = m.end()
        
        remaining = result[end_idx:]
        
        closing_quote_idx = None
        j = 0
        while j < len(remaining):
            if remaining[j] == '"':
                # Check if followed by JSON delimiter: , or } or ]
                is_delimiter = False
                k = j + 1
                while k < len(remaining):
                    c = remaining[k]
                    if c.isspace():
                        k += 1
                        continue
                    if c in (',', '}', ']'):
                        is_delimiter = True
                    break
                if is_delimiter:
                    closing_quote_idx = j
            
            # Check for another key-value start to stop scanning
            if j + 3 < len(remaining) and remaining[j] == '"':
                k = j + 1
                while k < len(remaining) and (remaining[k].isalnum() or remaining[k] in ('_', '-')):
                    k += 1
                if k < len(remaining) and remaining[k] == '"':
                    colon = k + 1
                    while colon < len(remaining) and remaining[colon].isspace():
                        colon += 1
                    if colon < len(remaining) and remaining[colon] == ':':
                        quote = colon + 1
                        while quote < len(remaining) and remaining[quote].isspace():
                            quote += 1
                        if quote < len(remaining) and remaining[quote] == '"':
                            break
            j += 1
            
        if closing_quote_idx is not None:
            raw_value = remaining[:closing_quote_idx]
            # Escape nested quotes
            repaired_value = []
            for idx, c in enumerate(raw_value):
                if c == '"':
                    is_escaped = idx > 0 and raw_value[idx-1] == '\\'
                    if not is_escaped:
                        repaired_value.append('\\')
                repaired_value.append(c)
            repaired_value = "".join(repaired_value)
            
            prefix = result[:end_idx]
            suffix = result[end_idx + closing_quote_idx:]
            result = prefix + repaired_value + suffix
            
    return result

s = r'{"tool":"run_command","arguments":{"command":"grep -rn "Initiate Meltdown" /Volumes/Corsair_Lab/Home/Projects/pbh-containment-sim/src/"}}'
repaired = repair_json_str(s)
print("Original:", s)
print("Repaired:", repaired)
try:
    print("Success:", json.loads(repaired))
except Exception as e:
    print("Failed:", e)

s2 = r'{"tool":"run_command","arguments":{"command":"grep -rn \"Already Escaped\" /src/"}}'
repaired2 = repair_json_str(s2)
print("Original 2:", s2)
print("Repaired 2:", repaired2)
try:
    print("Success 2:", json.loads(repaired2))
except Exception as e:
    print("Failed 2:", e)
