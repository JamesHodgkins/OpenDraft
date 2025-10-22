using CommunityToolkit.Mvvm.Input;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODGeometry;
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
        public ODDrawConnector DrawConnector => DrawConnectorRoot;

        public MainWindowViewModel()
        {
            ODSystem.Initialise();

            // Initialize the DataManager and Editor
            DataManagerRoot = new ODDataManager();
            InputService = new ODEditorInputService();
            EditorRoot = new ODEditor(DataManager, InputService);
            DrawConnectorRoot = new ODDrawConnector(DataManager, Editor);


            // Debug XSVG Reader
            ODXsvgReader xsvgR = new ODXsvgReader("XSVG/Example XSVG/example.xsvg");
            List<ODElement> xsvgElements = xsvgR.ExtractModelSpace();

            foreach (ODElement elem in xsvgElements)
            {
                Editor.AddStaticElement(elem);
            }

            Debug.WriteLine("Setup complete");
        }

        [RelayCommand]
        private void ExecuteEditor(string command)
        {
            EditorRoot.ExecuteCommand(command);
        }
        
    }
}