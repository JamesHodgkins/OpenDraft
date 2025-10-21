using CommunityToolkit.Mvvm.Input;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODGeometry;
using System.Collections.Generic;
using System.Collections.ObjectModel;

namespace OpenDraft.ViewModels
{
    public partial class MainWindowViewModel : ViewModelBase
    {
        public ODDataManager DataManager { get; }
        public IODEditorInputService InputService { get; }
        public ODEditor EditorRoot { get; private set; }

        public ObservableCollection<ODElement> GeometryElements => DataManager.Elements;
        public ODLayerManager LayerManager => DataManager.LayerManager;
        public ODEditor Editor => EditorRoot;

        public MainWindowViewModel()
        {
            ODSystem.Initialise();

            // Debug SVG Reader
            OpenDraft.XSVG.SvgImporter svgReader = new OpenDraft.XSVG.SvgImporter();
            svgReader.LoadSvgFile("XSVG/Example.svg");
            ODPoint svgSize = svgReader.GetDimensions();
            List<ODElement> svgElements = svgReader.ImportGeometry();

            // Initialize the DataManager and Editor
            DataManager = new ODDataManager();
            InputService = new ODEditorInputService();
            EditorRoot = new ODEditor(DataManager, InputService);

            // Set up layers (this can stay as-is)
            DataManager.LayerManager.AddLayer("New Layer");
            DataManager.LayerManager.SetActiveLayer("New Layer");

            ODLayer? lay = DataManager.LayerManager.GetLayerByName("New Layer");
            if (lay != null)
            {
                lay.Color = "#00FF00";
                lay.LineWeight = 2.0f;
                lay.LineType = "Dashed";
            }

        }


        [RelayCommand]
        private void ExecuteEditor(string command)
        {
            EditorRoot.ExecuteCommand(command);
        }
        
    }
}