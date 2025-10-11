import os

output_file = "combined_output.txt"

# Folders and file patterns to skip
skip_dirs = {"bin", "obj", ".vs", ".vscode", ".git"}
skip_exts = {".user", ".suo", ".log", ".tmp", ".cache", ".ide", ".Designer.cs"}

with open(output_file, "w", encoding="utf-8") as outfile:
    for root, dirs, files in os.walk("."):
        # Skip unwanted directories
        dirs[:] = [d for d in dirs if d not in skip_dirs]

        for file in files:
            # Skip unwanted file extensions
            if file.endswith(tuple(skip_exts)):
                continue

            # Only include .cs, .axaml, .xaml.cs
            if file.endswith((".cs", ".axaml")):
                path = os.path.join(root, file)
                outfile.write(f"\n\n** {file} **\n")
                outfile.write(f"Path: {path}\n\n")
                try:
                    with open(path, "r", encoding="utf-8") as infile:
                        outfile.write(infile.read())
                        outfile.write("\n\n\n")
                except Exception as e:
                    outfile.write(f"[Error reading {path}: {e}]\n\n\n")

print(f"All .cs and .axaml contents written to '{output_file}'")
