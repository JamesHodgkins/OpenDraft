using Avalonia;

public class ViewportCamera
{
    public Point Position { get; set; } = new Point(0, 0);
    public float Scale { get; set; } = 1.0f;

    public ViewportCamera(Point? position = null, float scale = 1.0f)
    {
        Position = position ?? new Point(0, 0);
        Scale = scale;
    }

    public void MoveBy(Vector delta) => Position += new Point(delta.X, delta.Y);

    public Matrix GetMatrix(double viewportHeight) =>
        new Matrix(Scale, 0, 0, -Scale, -Position.X * Scale, Position.Y * Scale + viewportHeight);
}