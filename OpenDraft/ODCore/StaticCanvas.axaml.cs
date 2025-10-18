using Avalonia;
using Avalonia.Controls;
using Avalonia.Markup.Xaml;
using Avalonia.Media;
using System;

namespace OpenDraft;

public partial class StaticCanvas : Control
{
    public Action<DrawingContext, Rect>? OnRender { get; set; }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        OnRender?.Invoke(context, Bounds);
    }
}
