// ODBaseEditorCommand.cs
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Reflection;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    public abstract class ODBaseEditorCommand : IODEditorCommand
    {
        private readonly Lazy<ODCommandAttribute> _attribute;

        public string Name => _attribute.Value.Name;
        public string Description => _attribute.Value.Description;

        protected ODBaseEditorCommand()
        {
            _attribute = new Lazy<ODCommandAttribute>(() =>
            {
                var attribute = GetType().GetCustomAttribute<ODCommandAttribute>();
                if (attribute == null)
                {
                    throw new InvalidOperationException(
                        $"Command {GetType().Name} must be decorated with [ODCommand] attribute");
                }
                return attribute;
            });
        }

        public abstract Task ExecuteAsync(IODEditorGateway editor);

        // Helper method for common validation
        protected void ValidatePoint(ODPoint point, string paramName)
        {
            if (point == null)
                throw new ArgumentNullException(paramName);

            if (double.IsNaN(point.X) || double.IsNaN(point.Y) ||
                double.IsInfinity(point.X) || double.IsInfinity(point.Y))
                throw new ArgumentException($"Invalid point coordinates: ({point.X}, {point.Y})", paramName);
        }
    }
}