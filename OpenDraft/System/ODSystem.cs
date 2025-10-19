using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;

namespace OpenDraft
{
    public static class ODSystem
    {
        private static Dictionary<string, string> _dictionary = new Dictionary<string, string>();


        public static void Initialise()
        {
            LoadRegistry();
        }

        private static void LoadRegistry()
        {
            _dictionary.Clear();
            string registryPath = "config.json";

            if (File.Exists(registryPath))
            {
                string json = File.ReadAllText(registryPath);
                using var doc = JsonDocument.Parse(json);

                var flat = new Dictionary<string, string>();
                FlattenJson(doc.RootElement, "", flat);

                foreach (var kvp in flat)
                    _dictionary[kvp.Key] = kvp.Value;
            }

            Debug.WriteLine($"ODSystem registry loaded with {_dictionary.Count} entries.");
        }

        private static void FlattenJson(JsonElement element, string prefix, Dictionary<string, string> result)
        {
            switch (element.ValueKind)
            {
                case JsonValueKind.Object:
                    foreach (var prop in element.EnumerateObject())
                    {
                        string newPrefix = string.IsNullOrEmpty(prefix) ? prop.Name : $"{prefix}/{prop.Name}";
                        FlattenJson(prop.Value, newPrefix, result);
                    }
                    break;

                case JsonValueKind.Array:
                    foreach (var item in element.EnumerateArray())
                    {
                        // Don’t append an index — treat array items as part of the same scope
                        FlattenJson(item, prefix, result);
                    }
                    break;

                default:
                    result[prefix] = element.ToString();
                    break;
            }
        }

        public static string? GetRegistryValueAsString(string key)
        {
            if (_dictionary.TryGetValue(key, out var value))
            {
                return value;
            }
            return null;
        }

        public static int? GetRegistryValueAsInt(string key)
        {
            if (_dictionary.TryGetValue(key, out var value))
            {
                if (int.TryParse(value, out var intValue))
                {
                    return intValue;
                }
            }
            return null;
        }

        public static bool? GetRegistryValueAsBool(string key)
        {
            if (_dictionary.TryGetValue(key, out var value))
            {
                if (bool.TryParse(value, out var boolValue))
                {
                    return boolValue;
                }
            }
            return null;
        }

        public static float ? GetRegistryValueAsDecimal(string key)
        {
            if (_dictionary.TryGetValue(key, out var value))
            {
                if (float.TryParse(value, out var floatValue))
                {
                    return floatValue;
                }
            }
            return null;
        }

        public static void SetRegistryValue(string key, string value)
        {
            _dictionary[key] = value;
            SaveRegistry();
        }

        private static void SaveRegistry()
        {
            // Save registry to file or other source
            // For example, write to a JSON file
            string registryPath = "config.json";
            var json = JsonSerializer.Serialize(_dictionary, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(registryPath, json);
            Debug.WriteLine("ODSystem registry saved with " + _dictionary.Count + " entries.");
        }

    }

}
