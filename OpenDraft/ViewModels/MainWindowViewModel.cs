using OpenDraft.Core.ODData;
using OpenDraft.Core.ODGeometry;
using System.Collections.Generic;
using static System.Runtime.InteropServices.JavaScript.JSType;

namespace OpenDraft.ViewModels
{
    public class MainWindowViewModel : ViewModelBase
    {
        public ODDataManager DataManager { get; }

        public IEnumerable<ODElement> GeometryElements => DataManager.Elements;
        public ODLayerManager LayerManager => DataManager.LayerManager;


        public MainWindowViewModel()
        {
            // 3. Initialize the DataManager.
            DataManager = new ODDataManager();

            // You can add some sample data for testing here.
            // For example: DataManager.Elements.Add(new ODLine(...));

            
            // DEBUG SOME GEOMETRY
            ODLine line1 = new ODLine(new ODPoint(-100, -100), new ODPoint(-100, 100));
            DataManager.AddElement(line1);

            ODLine line2 = new ODLine(new ODPoint(-100, 100), new ODPoint(100, 100));
            DataManager.AddElement(line2);

            ODLine line3 = new ODLine(new ODPoint(100, 100), new ODPoint(100, -100));
            DataManager.AddElement(line3);

            ODLine line4 = new ODLine(new ODPoint(100, -100), new ODPoint(-100, -100));
            DataManager.AddElement(line4);


            DataManager.LayerManager.AddLayer("New Layer");
            DataManager.LayerManager.SetActiveLayer("New Layer");
            
            ODLayer? lay = DataManager.LayerManager.GetLayerByName("New Layer");

            if (lay != null)
                lay.Color = "#00FF00";

            // Origin Crosshair
            ODLine lineX = new ODLine(new ODPoint(-20, 0), new ODPoint(20, 0));
            DataManager.AddElement(lineX);

            ODLine lineY = new ODLine(new ODPoint(0, -20), new ODPoint(0, 20));
            DataManager.AddElement(lineY);


        }
    }
}
