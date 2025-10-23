using Avalonia.Media;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System.Collections.Generic;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODSymbolDefinition : ODElement
    {
        public string Name { get; set; }
        public List<ODElement> Elements { get; set; } = new List<ODElement>();

        public ODSymbolDefinition(string name)
        {
            Name = name;
        }
    }
}