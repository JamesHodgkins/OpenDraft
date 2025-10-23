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
    }
}
