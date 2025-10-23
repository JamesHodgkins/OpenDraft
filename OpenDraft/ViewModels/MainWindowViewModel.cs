using CommunityToolkit.Mvvm.Input;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using OpenDraft.ODCore.ODSystem;
using OpenDraft.XSVG;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;

namespace OpenDraft.ViewModels
{
    public partial class MainWindowViewModel : ViewModelBase
    {
        public ODDataManager DataManagerRoot { get; }
        public IODEditorInputService InputService { get; }
        public ODEditor EditorRoot { get; private set; }
        public ODDrawConnector DrawConnectorRoot { get; }

        public ObservableCollection<ODElement> GeometryElements => DataManager.Elements;
        public ODDataManager DataManager => DataManagerRoot;
        public ODEditor Editor => EditorRoot;
        public ODSymbolTable SymbolTable { get; private set; }
        public ODDrawConnector DrawConnector => DrawConnectorRoot;

        public MainWindowViewModel()
        {
            ODSystem.Initialise();

            // Initialize the DataManager and Editor
            DataManagerRoot = new ODDataManager();
            InputService = new ODEditorInputService();
            EditorRoot = new ODEditor(DataManager, InputService);
            SymbolTable = new ODSymbolTable();
            DrawConnectorRoot = new ODDrawConnector(DataManager, Editor, SymbolTable);


            // Debug XSVG Reader
            /*ODXsvgReader xsvgR = new ODXsvgReader("XSVG/Example XSVG/example.xsvg");
            List<ODElement> xsvgElements = xsvgR.ExtractModelSpace();

            foreach (ODElement elem in xsvgElements)
            {
                Editor.AddStaticElement(elem);
            }*/

            /* Create test layer */
            DataManager.LayerManager.AddLayer("New Layer");
            ODLayer? lay = DataManager.LayerManager.GetLayerByName("New Layer");
            DataManager.LayerManager.SetActiveLayer("New Layer");
            lay!.Color = new ODColour("#00FF00");
            lay!.LineWeight = 2.0f;

            Debug.WriteLine("Setup complete");


            // Test Symbols
            ODSymbolDefinition symDef = new ODSymbolDefinition("MySymbol");
            ODLine L1 = new ODLine(new ODVec2(0, 0), new ODVec2(1, 1));
            ODLine L2 = new ODLine(new ODVec2(0, 1), new ODVec2(1, 0));

            symDef.Elements.Add(L1);
            symDef.Elements.Add(L2);
            SymbolTable.AddSymbol(symDef);

            ODSymbol S1 = new ODSymbol("MySymbol", new ODVec2(10, -10));
            ODSymbol S2 = new ODSymbol("MySymbol", new ODVec2(25, -10));

            DataManager.AddElement(S1);
            DataManager.AddElement(S2);
        }

        [RelayCommand]
        private void ExecuteEditor(string command)
        {
            EditorRoot.ExecuteCommand(command);
        }
        
    }
}