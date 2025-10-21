import os

OUTPUT_FILE = "combined_output.txt"
SKIP_DIRS = {"bin", "obj", ".vs", ".vscode", ".git"}
SKIP_EXTS = {".user", ".suo", ".log", ".tmp", ".cache", ".ide", ".Designer.cs"}
INCLUDE_EXTS = {".cs", ".axaml"}

def should_skip(file):
    return file.endswith(tuple(SKIP_EXTS))

def is_included(file):
    return file.endswith(tuple(INCLUDE_EXTS))

def write_file_contents(outfile, path, filename):
    outfile.write(f"\n\n** {filename} **\n")
    outfile.write(f"Path: {os.path.abspath(path)}\n\n")
    try:
        with open(path, "r", encoding="utf-8") as infile:
            for line in infile:
                if line.strip():  # skip blank lines
                    outfile.write(line)
            outfile.write("\n\n")
    except Exception as e:
        outfile.write(f"[Error reading {path}: {e}]\n\n")

def main():
    with open(OUTPUT_FILE, "w", encoding="utf-8") as outfile:
        for root, dirs, files in os.walk("."):
            # Skip unwanted directories
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]

            for file in files:
                if should_skip(file) or not is_included(file):
                    continue
                write_file_contents(outfile, os.path.join(root, file), file)

    print(f"All .cs and .axaml contents written to '{OUTPUT_FILE}'")

if __name__ == "__main__":
    main()
