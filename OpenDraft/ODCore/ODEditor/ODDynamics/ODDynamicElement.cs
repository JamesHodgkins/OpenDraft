using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODDynamicElement
    {
        public Guid Id { get; set; }
        public string Color { get; set; } = "#FF0000";
        public float LineWeight { get; set; } = 2.0f;

        public ODDynamicElement()
        {
            Id = Guid.NewGuid();
        }

        public virtual void Draw(DrawingContext context, float scale, ODPoint vpWorldSize, ODPoint mousePosition)
        {
            // Base draw method, to be overridden by derived classes
        }

    }
}
