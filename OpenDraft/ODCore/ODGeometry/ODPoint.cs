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
    public class ODPoint : ODElement
    {
        public double X { get; set; }
        public double Y { get; set; }
        public ODPoint(double x, double y)
        {
            X = x;
            Y = y;
        }

        public ODPoint(float x, float y)
        {
            X = x;
            Y = y;
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector)
        {
            // To Do
        }

        public override ODBoundingBox GetBoundingBox()
        {
            return new ODBoundingBox(new ODVec2(0, 0), new ODVec2(0, 0));
        }

        public override bool HitTest(ODVec2 point)
        {
            return false;
        }
    }
}
