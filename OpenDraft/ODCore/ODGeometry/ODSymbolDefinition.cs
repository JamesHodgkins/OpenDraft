using OpenDraft.ODCore.ODGeometry;
using System.Collections.Generic;
using OpenDraft.ODCore.ODMath;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODSymbol : ODElement
    {
        public string Name { get; set; }
        public List<ODElement> Elements { get; set; } = new List<ODElement>();

        public ODSymbol(string name, ODVec2 basePoint)
        {
            Name = name;
        }

    }

}