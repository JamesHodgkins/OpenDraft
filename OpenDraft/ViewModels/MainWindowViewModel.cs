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
        public ODDataService DataServiceRoot { get; }
        public ODEditorHandler EditorHandlerRoot { get; }

        public ObservableCollection<ODElement> GeometryElements => DataManager.Elements;
        public ODDataManager DataManager => DataManagerRoot;
        public ODEditor Editor => EditorRoot;
        public ODSymbolTable SymbolTable { get; private set; }
        public ODDataService DataService => DataServiceRoot;
        public ODEditorHandler EditorHandler => EditorHandlerRoot;

        public MainWindowViewModel()
        {
            ODSystem.Initialise();

            // Initialize the DataManager and Editor
            DataManagerRoot = new ODDataManager();
            InputService = new ODEditorInputService();
            EditorRoot = new ODEditor(DataManager, InputService);
            SymbolTable = new ODSymbolTable();
            DataServiceRoot = new ODDataService(DataManager, Editor, SymbolTable);
            EditorHandlerRoot = new ODEditorHandler(EditorRoot);


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
            lay!.LineWeight = .5f;

        }

        [RelayCommand]
        private void ExecuteEditor(string command)
        {
            EditorRoot.ExecuteCommand(command);
        }
        
    }
}