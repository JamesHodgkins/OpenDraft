using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODData
{
    public class ODLayerManager
    {
        private List<ODLayer> Layers { get; set; } = new List<ODLayer>();
        private ushort ActiveLayer { get; set; }
        public ODLineStyleRegistry LineStyleRegistry { get; } = new ODLineStyleRegistry();

        public ODLayerManager()
        {
            // Add a default layer
            AddLayer("Default");

            AddLayer("Crosshair X");
            GetLayerByName("Crosshair X")!.IsSystemLayer = true;
            AddLayer("Crosshair Y");
            GetLayerByName("Crosshair Y")!.IsSystemLayer = true;
            AddLayer("Crosshair Square");
            GetLayerByName("Crosshair Square")!.IsSystemLayer = true;


        }


        private ushort GetNextAvailableID()
        {
            HashSet<ushort> usedIDs = new HashSet<ushort>(Layers.Select(layer => layer.getID()));
            for (ushort id = 0; id < ushort.MaxValue; id++)
            {
                if (!usedIDs.Contains(id))
                    return id;
            }
            throw new InvalidOperationException("No available layer IDs.");
        }


        public ushort GetActiveLayer()
        {
            return ActiveLayer;
        }


        public ushort AddLayer(string name)
        {
            ushort newID = GetNextAvailableID();

            ODLayer newLayer = new ODLayer(newID, name);
            Layers.Add(newLayer);
            return newID;
        }

        public ODLayer? GetLayerByName(string name)
        {
            return Layers.FirstOrDefault(layer => layer.Name == name);
        }

        public ODLayer? GetLayerByID(ushort id)
        {
            return Layers.FirstOrDefault(layer => layer.getID() == id);
        }

        public void SetActiveLayer(string name)
        {
            ODLayer? layer = GetLayerByName(name);
            if (layer != null)
            {
                ActiveLayer = layer.getID();
            }
            else
            {
                throw new ArgumentException($"Layer with name '{name}' does not exist.");
            }
        }

        public void RemoveLayer(string name)
        {
            ODLayer? layerToRemove = GetLayerByName(name);

            if (layerToRemove != null)
            {
                if (layerToRemove.IsSystemLayer)
                {
                    throw new InvalidOperationException("Cannot remove a system layer.");
                }

                if (ActiveLayer == layerToRemove.getID())
                {
                    SetActiveLayer("Default");
                }
                
                Layers.Remove(layerToRemove);
            }
        }
    }
}
