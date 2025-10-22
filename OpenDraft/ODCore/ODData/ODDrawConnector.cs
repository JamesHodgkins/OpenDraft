using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Avalonia.Media;
using OpenDraft.ODCore.ODEditor;

namespace OpenDraft.ODCore.ODData
{
    public class ODDrawConnector
    {
        private ODDataManager DataManager;
        private ODEditor.ODEditor? Editor;
        
        public ODDrawConnector(ODDataManager dm, ODEditor.ODEditor ed)
        {
            DataManager = dm;
            Editor = ed;
        }

        public ODLayer? GetLayerByName(string name) {
            return this.DataManager.LayerManager.GetLayerByName(name);
        }

        public ODLayer? GetLayerByID(ushort id) {
            return this.DataManager.LayerManager.GetLayerByID(id);
        }

        public IDashStyle ToAvaloniaDashStyle(string dashStyle)
        {
            return DataManager.LineStyleRegister.ToAvaloniaDashStyle(dashStyle);
        }
    }
}
