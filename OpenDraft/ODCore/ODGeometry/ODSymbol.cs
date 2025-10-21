using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System.Collections.Generic;
using System.Net;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODSymbolDefinition : ODElement
    {
        public string BlockName { get; set; }
        public ODPoint InsertionPoint { get; set; }
        public double Rotation { get; set; }
        public double ScaleX { get; set; } = 1.0;
        public double ScaleY { get; set; } = 1.0;


        public ODSymbolDefinition(string blockName, ODPoint insertionPoint)
        {
            BlockName = blockName;
            InsertionPoint = insertionPoint;
        }


        public override void Draw(DrawingContext context, ODDataManager dm)
        {
            ODLayer? layer = dm.LayerManager.GetLayerByID(LayerId);
            ODLineStyleRegistry lsRegistry = dm.LayerManager.LineStyleRegistry;

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            IDashStyle dashStyle = lsRegistry.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color;
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            Pen pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            //foreach (ODElement element in GetBlockElements(BlockName))
            //{
                //element.Draw(context, lm);
            //}
        }
    }
}