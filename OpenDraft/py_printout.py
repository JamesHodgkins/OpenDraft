import os
import sys

OUTPUT_FILE = "combined_output.txt"
SKIP_DIRS = {"bin", "obj", ".vs", ".vscode", ".git"}
SKIP_EXTS = {".user", ".suo", ".log", ".tmp", ".cache", ".ide", ".Designer.cs"}
INCLUDE_EXTS = {".cs", ".axaml"}


def should_skip(file):
    return file.endswith(tuple(SKIP_EXTS))


def is_included(file):
    return file.endswith(tuple(INCLUDE_EXTS))


def file_contains_phrase(path, phrase):
    if not phrase:
        return True  # include all
    try:
        with open(path, "r", encoding="utf-8") as f:
            return any(phrase in line for line in f)
    except Exception:
        return False


def write_file_contents(outfile, path, filename):
    outfile.write(f"\n\n** {filename} **\n")
    outfile.write(f"Path: {os.path.abspath(path)}\n\n")
    try:
        with open(path, "r", encoding="utf-8") as infile:
            for line in infile:
                if line.strip():
                    outfile.write(line)
    except Exception as e:
        outfile.write(f"[Error reading {path}: {e}]\n\n")


def main():
    phrase = sys.argv[1] if len(sys.argv) > 1 else ""
    with open(OUTPUT_FILE, "w", encoding="utf-8") as outfile:
        for root, dirs, files in os.walk("."):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for file in files:
                if should_skip(file) or not is_included(file):
                    continue
                full_path = os.path.join(root, file)
                if file_contains_phrase(full_path, phrase):
                    write_file_contents(outfile, full_path, file)

    if phrase:
        print(f"Files containing '{phrase}' written to '{OUTPUT_FILE}'")
    else:
        print(f"All .cs and .axaml contents written to '{OUTPUT_FILE}'")


if __name__ == "__main__":
    main()
