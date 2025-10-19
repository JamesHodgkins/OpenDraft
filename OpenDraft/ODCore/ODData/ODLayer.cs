using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODData
{
    public class ODLayer
    {
        private ushort _id;
        public String Name { get; set; } = Guid.NewGuid().ToString();
        public String Color { get; set; } = "#FFFFFF";
        public String LineType { get; set; } = "Continuous";
        public float LineWeight { get; set; } = 0.6f;
        public bool IsVisible { get; set; } = true;
        public bool IsLocked { get; set; } = false;
        public bool IsSystemLayer { get; set; } = false;

        public ODLayer(ushort id, String name)
        {
            _id = id;
            Name = name;
        }

        public ushort getID()
        {
            return _id;
        }
    }
}
