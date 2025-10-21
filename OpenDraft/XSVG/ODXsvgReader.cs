using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Linq;

namespace OpenDraft.XSVG
{
    internal class ODXsvgReader
    {
        private static readonly List<string> _tempDirectories = new List<string>();
        private readonly string _filePath;
        private readonly string _tempPath;
        private bool _isValid = false;

        public static void CleanupAllTempFiles()
        {
            Debug.WriteLine($"Cleaning up {_tempDirectories.Count} temporary directories...");

            foreach (string dir in _tempDirectories.ToList())
            {
                try
                {
                    if (Directory.Exists(dir))
                    {
                        Directory.Delete(dir, true);
                        Debug.WriteLine($"Cleaned up temp directory: {dir}");
                    }
                    _tempDirectories.Remove(dir);
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"Failed to delete temp directory {dir}: {ex.Message}");
                }
            }

            Debug.WriteLine("Cleanup completed.");
        }

        public static bool IsValidXSVGFile(string filePath)
        {
            if (string.IsNullOrEmpty(filePath))
                return false;

            if (!File.Exists(filePath))
            {
                Debug.WriteLine($"File does not exist: {filePath}");
                return false;
            }

            try
            {
                FileInfo fileInfo = new FileInfo(filePath);
                if (fileInfo.Length == 0)
                {
                    Debug.WriteLine($"File is empty: {filePath}");
                    return false;
                }

                // Check file signature (first 4 bytes of ZIP file)
                byte[] zipSignature = new byte[] { 0x50, 0x4B, 0x03, 0x04 };
                byte[] fileSignature = new byte[4];

                using (FileStream fs = new FileStream(filePath, FileMode.Open, FileAccess.Read))
                {
                    if (fs.Read(fileSignature, 0, 4) != 4)
                    {
                        Debug.WriteLine($"Cannot read file signature: {filePath}");
                        return false;
                    }
                }

                if (!fileSignature.SequenceEqual(zipSignature))
                {
                    Debug.WriteLine($"File is not a ZIP file (invalid signature): {filePath}");
                    return false;
                }

                return true;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"File validation failed for {filePath}: {ex.Message}");
                return false;
            }
        }

        public ODXsvgReader(string filePath)
        {
            _filePath = filePath ?? throw new ArgumentNullException(nameof(filePath));

            Debug.WriteLine($"Attempting to open XSVG file: {_filePath}");

            // Step 1: Check if file exists
            if (!File.Exists(_filePath))
            {
                throw new FileNotFoundException($"XSVG file not found: {_filePath}");
            }

            // Step 2: Check file size
            FileInfo fileInfo = new FileInfo(_filePath);
            if (fileInfo.Length == 0)
            {
                throw new InvalidDataException($"XSVG file is empty: {_filePath}");
            }

            // Step 3: Create temp directory
            _tempPath = Path.Combine(Path.GetTempPath(), "OpenDraft2D", Guid.NewGuid().ToString());
            Directory.CreateDirectory(_tempPath);
            _tempDirectories.Add(_tempPath);

            Debug.WriteLine($"Created temp directory: {_tempPath}");

            try
            {
                // Step 4: Extract
                Debug.WriteLine("Extracting ZIP contents...");
                ZipFile.ExtractToDirectory(_filePath, _tempPath);

                // Step 5: Validate XSVG structure
                Debug.WriteLine("Validating XSVG structure...");
                ValidateXSVGStructure();

                _isValid = true;
                Debug.WriteLine($"XSVGReader successfully loaded: {_filePath}");
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Failed to process XSVG file: {ex.Message}");
                CleanupThisTempDirectory();
                throw new IOException($"Failed to read XSVG file '{_filePath}': {ex.Message}", ex);
            }
        }

        public List<ODElement> ExtractModelSpace()
        {
            Debug.WriteLine("Begining parsing of files...");

            ODSvgReader svgR = new ODSvgReader();
            svgR.LoadSvgFile(GetFullPath("mod/ms.svg"));
            return svgR.ImportGeometry();
        }

        private void ValidateXSVGStructure()
        {
            // Check for required XSVG directories and files
            string[] requiredDirs = { "lay", "meta", "mod", "res" };
            string[] requiredFiles = { "manifest.json", "meta/meta.json", "mod/ms.svg" };

            foreach (string dir in requiredDirs)
            {
                string dirPath = Path.Combine(_tempPath, dir);
                if (!Directory.Exists(dirPath))
                {
                    throw new InvalidDataException($"Required directory '{dir}' not found in XSVG file");
                }
            }

            foreach (string file in requiredFiles)
            {
                string filePath = Path.Combine(_tempPath, file);
                if (!File.Exists(filePath))
                {
                    throw new InvalidDataException($"Required file '{file}' not found in XSVG file");
                }
            }

            Debug.WriteLine("XSVG structure validation passed");
        }

        public string TempPath
        {
            get
            {
                if (!_isValid) throw new InvalidOperationException("XSVGReader is not in a valid state");
                return _tempPath;
            }
        }

        public string FilePath => _filePath;
        public bool IsValid => _isValid;

        public string GetFullPath(string relativePath)
        {
            if (!_isValid)
                throw new InvalidOperationException("XSVGReader is not in a valid state");

            string fullPath = Path.Combine(_tempPath, relativePath);

            if (!File.Exists(fullPath) && !Directory.Exists(fullPath))
                throw new FileNotFoundException($"Path '{relativePath}' not found in XSVG file");

            return fullPath;
        }

        public bool PathExists(string relativePath)
        {
            if (!_isValid) return false;

            string fullPath = Path.Combine(_tempPath, relativePath);
            return File.Exists(fullPath) || Directory.Exists(fullPath);
        }

        public string ReadManifest()
        {
            return File.ReadAllText(GetFullPath("manifest.json"));
        }

        public string ReadMetaJson()
        {
            return File.ReadAllText(GetFullPath("meta.json"));
        }

        public string[] GetLayerFiles()
        {
            if (!_isValid) return Array.Empty<string>();

            string layDir = Path.Combine(_tempPath, "lay");
            return Directory.Exists(layDir)
                ? Directory.GetFiles(layDir)
                : Array.Empty<string>();
        }

        public string[] GetResourceFiles()
        {
            if (!_isValid) return Array.Empty<string>();

            string resDir = Path.Combine(_tempPath, "res");
            return Directory.Exists(resDir)
                ? Directory.GetFiles(resDir)
                : Array.Empty<string>();
        }

        public string GetModelSvg()
        {
            return File.ReadAllText(GetFullPath("mod/ms.svg"));
        }

        private void CleanupThisTempDirectory()
        {
            try
            {
                if (!string.IsNullOrEmpty(_tempPath) && Directory.Exists(_tempPath))
                {
                    Directory.Delete(_tempPath, true);
                    _tempDirectories.Remove(_tempPath);
                    Debug.WriteLine($"Cleaned up temp directory: {_tempPath}");
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Warning: Failed to clean up temp directory {_tempPath}: {ex.Message}");
            }
        }

        // Optional: Method to clean up this specific instance if needed before app closure
        public void Cleanup()
        {
            CleanupThisTempDirectory();
        }
    }
}