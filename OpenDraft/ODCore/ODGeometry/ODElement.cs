using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODElement
    {
        public Guid Id { get; set; }
        public ushort LayerId { get; set; } = 0; // Default layer ID
        public ODColour? Colour { get; set; } = null; // If null, use layer colour
        public float? LineWeight { get; set; } = null; // If null, use layer lineweight
        public string? LineType { get; set; } = null; // If null, use layer linetype

        public ODElement()
        {
            Id = Guid.NewGuid();
        }

        public virtual void Draw(DrawingContext context, ODDrawConnector connector)
        {
            // Base draw method, to be overridden by derived classes
        }

    }
}
