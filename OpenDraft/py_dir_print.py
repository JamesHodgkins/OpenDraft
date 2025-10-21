import os

def write_tree(root_path, outfile, prefix=""):
    entries = sorted(os.listdir(root_path))
    entries_count = len(entries)
    
    for i, entry in enumerate(entries):
        path = os.path.join(root_path, entry)
        connector = "└── " if i == entries_count - 1 else "├── "
        outfile.write(prefix + connector + entry + "\n")
        if os.path.isdir(path):
            extension = "    " if i == entries_count - 1 else "│   "
            write_tree(path, outfile, prefix + extension)

if __name__ == "__main__":
    output_file = "directory_tree.txt"
    root_dir = "."  # Change this to the directory you want to scan

    with open(output_file, "w", encoding="utf-8") as f:
        f.write(root_dir + "\n")
        write_tree(root_dir, f)

    print(f"Directory tree written to '{output_file}'")
