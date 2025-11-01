using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODDynamicElement
    {
        public Guid Id { get; set; }
        public ODColour? Colour { get; set; } = null; // If null, use layer colour
        public double? LineWeight { get; set; } = 2.0f; // If null, use layer line weight
        public string? LineType { get; set; } = null; // If null, use layer line type
        public ushort LayerId { get; set; } = 0; // Default layer ID

        public ODDynamicElement()
        {
            Id = Guid.NewGuid();
        }

        public virtual void Draw(DrawingContext context, ODDataService connector, 
            double scale, ODVec2 vpWorldSize, ODVec2 mousePosition)
        {
            // Base draw method, to be overridden by derived classes
        }

    }
}
