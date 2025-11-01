using Avalonia.Media;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;

namespace OpenDraft.ODCore.ODData
{
    public class ODDataService : IODDataService
    {
        private ODDataManager DataManager;
        private ODEditor.ODEditor? Editor;
        private ODSymbolTable SymbolTable;
        
        public ODDataService(ODDataManager dm, ODEditor.ODEditor ed, ODSymbolTable st)
        {
            DataManager = dm;
            Editor = ed;
            SymbolTable = st;
        }

        public ODLayer? GetLayerByName(string name) {
            return this.DataManager.LayerManager.GetLayerByName(name);
        }

        public ODLayer? GetLayerByID(ushort id) {
            return this.DataManager.LayerManager.GetLayerByID(id);
        }

        public IDashStyle ToAvaloniaDashStyle(string dashStyle)
        {
            ODLineStyleRegistry lsr = DataManager.LineStyleRegister;
            return lsr.ToAvaloniaDashStyle(dashStyle) ?? lsr.ToAvaloniaDashStyle("Continous")!;
        }

        public ODSymbolDefinition? GetSymbolDefinition(string symbolName)
        {
            return SymbolTable.GetSymbolDefinition(symbolName);
        }
    }
}
