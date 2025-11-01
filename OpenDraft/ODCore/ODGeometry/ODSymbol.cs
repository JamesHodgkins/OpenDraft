using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODSymbol : ODElement
    {
        public string SymbolName { get; set; }
        public ODVec2 InsertionPoint { get; set; }
        public double Rotation { get; set; }
        public double Scale { get; set; } = 1.0;


        public ODSymbol(string symbolName, ODVec2 insertionPoint)
        {
            SymbolName = symbolName;
            InsertionPoint = insertionPoint;
        }


        private Matrix CreateSymbolMatrix()
        {
            // Start with identity matrix
            var matrix = Matrix.Identity;

            // Apply scaling
            if (Scale != 1.0)
            {
                matrix = matrix * Matrix.CreateScale(Scale, Scale);
            }

            // Apply rotation (convert degrees to radians)
            if (Rotation != 0.0)
            {
                double radians = Rotation * Math.PI / 180.0;
                matrix = matrix * Matrix.CreateRotation(radians);
            }

            // Apply translation
            matrix = matrix * Matrix.CreateTranslation(new Vector(InsertionPoint.X, InsertionPoint.Y));

            return matrix;
        }


        public override void Draw(DrawingContext context, ODDataService connector)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            var dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            ODColour effectiveColour = (Colour != null) ? Colour : layer.Color;
            double effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            var pen = new Pen(
                new SolidColorBrush(effectiveColour.ToAvaloniaColor()), // Brush only
                effectiveLineWeight,                                    // Thickness
                dashStyle                                               // Dash style
            );


            ODSymbolDefinition? symbolDefition = connector.GetSymbolDefinition(SymbolName);

            if (symbolDefition == null)
                return;

            using (context.PushTransform(CreateSymbolMatrix()))
            {
                foreach (ODElement element in symbolDefition.Elements)
                {
                    element.Draw(context, connector);
                }
            }
        }

        public override void DrawHighlight(DrawingContext context, ODDataService connector, ODColour hColour, int hIntensity)
        {

        }

        public override ODBoundingBox GetBoundingBox()
        {
            return new ODBoundingBox(new ODVec2(0, 0), new ODVec2(0, 0));
            
        }

        public override bool HitTest(ODVec2 point, double tolerance)
        {
            return false;
        }

    }
}