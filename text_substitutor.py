import os
import re

def replace_in_file(file_path, old_word, new_word):
    try:
        with open(file_path, 'r') as f:
            content = f.read()
        new_content = re.sub(r'\b' + re.escape(old_word) + r'\b', new_word, content)
        with open(file_path, 'w') as f:
            f.write(new_content)
        print(f'Replaced "{old_word}" with "{new_word}" in {file_path}')
    except Exception as e:
        print(f'Error processing {file_path}: {str(e)}')

if __name__ == '__main__':
    import sys
    if len(sys.argv) < 3:
        print('Usage: python text_substitutor.py <old_word> <new_word> [directory]')
        sys.exit(1)

    old_word = sys.argv[1]
    new_word = sys.argv[2]
    dir_path = sys.argv[3] if len(sys.argv) > 3 else '.'

    for root, dirs, files in os.walk(dir_path):
        for file in files:
            if file.endswith('.txt'):
                full_path = os.path.join(root, file)
                replace_in_file(full_path, old_word, new_word)