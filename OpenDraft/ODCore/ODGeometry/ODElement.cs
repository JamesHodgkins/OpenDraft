using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODMath;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODGeometry
{
    public abstract class ODElement
    {
        public Guid Id { get; set; }
        public ushort LayerId { get; set; } = 0; // Default layer ID
        public ODColour? Colour { get; set; } = null; // If null, use layer colour
        public double? LineWeight { get; set; } = null; // If null, use layer lineweight
        public string? LineType { get; set; } = null; // If null, use layer linetype

        public ODElement()
        {
            Id = Guid.NewGuid();
        }

        public abstract void Draw(DrawingContext context, ODDrawConnector connector);
        public abstract void DrawHighlight(DrawingContext context, ODDrawConnector connector, ODColour hColour, int hIntensity);
        public abstract ODBoundingBox GetBoundingBox();
        public abstract bool HitTest(ODVec2 point, double tolerance);

        public bool isPointInsideBoundingBox(ODVec2 point)
        {
            ODBoundingBox bb = GetBoundingBox();
            if (point.X >= bb.Left && point.X <= bb.Right && point.Y >= bb.Bottom && point.Y <= bb.Top)
                return true;
            return false;
        }

    }
}
