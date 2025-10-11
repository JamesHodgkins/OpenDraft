using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.Core.ODGeometry
{
    internal class ODPoint : ODElement
    {
        public float X { get; set; }
        public float Y { get; set; }
        public ODPoint(double x, double y)
        {
            X = (float)x;
            Y = (float)y;
        }

        public ODPoint(float x, float y)
        {
            X = x;
            Y = y;
        }
    }
}
