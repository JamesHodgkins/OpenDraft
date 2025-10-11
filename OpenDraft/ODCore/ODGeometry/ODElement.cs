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

        public ODElement()
        {
            Id = Guid.NewGuid();
        }

        public virtual void Draw(DrawingContext context, ODLayer layer)
        {
            // Base draw method, to be overridden by derived classes
        }

    }
}
