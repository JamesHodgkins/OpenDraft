using Avalonia.Media;
using System;
using System.Collections.Generic;
using System.Linq;

namespace OpenDraft.ODCore.ODData
{
    public class ODLineStyleRegistry
    {
        private Dictionary<string, string> lineStyles = new Dictionary<string, string>();
        private Dictionary<string, float[]> lineStyleArrays = new Dictionary<string, float[]>();

        public ODLineStyleRegistry()
        {
            InitialiseDefaultLineStyles();
        }

        private void InitialiseDefaultLineStyles()
        {
            // Store both string and array representations
            RegisterLineStyle("Continuous", "");
            RegisterLineStyle("Dashed", "10,5");
            RegisterLineStyle("Dotted", "2,5");
            RegisterLineStyle("DashDot", "10,5,2,5");
            RegisterLineStyle("DashDotDot", "10,5,2,5,2,5");
            RegisterLineStyle("Center", "20,5,2,5,2,5");
            RegisterLineStyle("Phantom", "25,5,2,5,2,5,2,5");
            RegisterLineStyle("Border", "15,3");
            RegisterLineStyle("Hidden", "5,3");
        }

        public string? GetStyleAsString(string name)
        {
            if (lineStyles.ContainsKey(name))
            {
                return lineStyles[name];
            }
            return null;
        }

        public float[]? GetStyleAsArray(string name)
        {
            if (lineStyleArrays.ContainsKey(name))
            {
                return lineStyleArrays[name];
            }
            return null;
        }

        public void RegisterLineStyle(string name, string dashArray)
        {
            lineStyles[name] = dashArray;

            // Pre-compute the array version for performance
            if (!string.IsNullOrEmpty(dashArray))
            {
                var parts = dashArray.Split(',');
                var array = new float[parts.Length];
                for (int i = 0; i < parts.Length; i++)
                {
                    if (float.TryParse(parts[i], out float value))
                    {
                        array[i] = value;
                    }
                    else
                    {
                        array[i] = 0;
                    }
                }
                lineStyleArrays[name] = array;
            }
            else
            {
                lineStyleArrays[name] = Array.Empty<float>();
            }
        }

        public void RegisterLineStyle(string name, float[] dashArray)
        {
            lineStyleArrays[name] = dashArray;
            lineStyles[name] = dashArray.Length > 0 ? string.Join(",", dashArray) : "";
        }

        public bool UnregisterLineStyle(string name)
        {
            bool removed1 = lineStyles.Remove(name);
            bool removed2 = lineStyleArrays.Remove(name);
            return removed1 || removed2;
        }

        public IEnumerable<string> GetAvailableStyleNames()
        {
            return lineStyles.Keys.OrderBy(name => name);
        }

        public bool StyleExists(string name)
        {
            return lineStyles.ContainsKey(name);
        }

        // Convert to Avalonia DashStyle
        public IDashStyle ToAvaloniaDashStyle(string linetypeName)
        {
            var pattern = GetStyleAsArray(linetypeName);
            if (pattern == null || pattern.Length == 0)
                return null; // Continuous line

            return new DashStyle(pattern.Select(p => (double)p).ToArray(), 0);
        }

        // Parse from SVG dash array
        public string? FromSvgDashArray(string dashArray)
        {
            if (string.IsNullOrEmpty(dashArray) || dashArray == "none")
                return "Continuous";

            // Try to find exact match first
            var exactMatch = lineStyles.FirstOrDefault(x => x.Value == dashArray).Key;
            if (exactMatch != null)
                return exactMatch;

            // Try to parse and find similar
            try
            {
                var parts = dashArray.Split(',');
                var pattern = parts.Select(p => float.Parse(p.Trim())).ToArray();

                // Look for matching pattern
                foreach (var style in lineStyleArrays)
                {
                    if (style.Value.SequenceEqual(pattern))
                        return style.Key;
                }

                // No match found, create a custom style
                string customName = $"Custom_{dashArray.Replace(",", "_")}";
                RegisterLineStyle(customName, pattern);
                return customName;
            }
            catch
            {
                return "Continuous"; // Fallback
            }
        }
    }
}