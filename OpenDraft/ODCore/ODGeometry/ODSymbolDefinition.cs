using OpenDraft.ODCore.ODGeometry;
using System.Collections.Generic;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODSymbol : ODElement
    {
        public string Name { get; set; }
        public List<ODElement> Elements { get; set; } = new List<ODElement>();

        public ODSymbol(string name, ODPoint basePoint)
        {
            Name = name;
        }

    }

}