using OpenDraft.ODCore.ODMath;

public class ODBoundingBox
{
    private ODVec2 _origin;  // This should be the MINIMUM point (bottom-left in Y-up system)
    private ODVec2 _size;

    public ODBoundingBox(ODVec2 origin, ODVec2 size)
    {
        _origin = origin;
        _size = size;
    }

    public ODVec2 GetOrigin => _origin;
    public ODVec2 GetSize => _size;
    public double Width => _size.X;
    public double Height => _size.Y;

    // Fixed corner calculations
    public ODVec2 BottomLeft => _origin;  // Minimum X, Minimum Y
    public ODVec2 BottomRight => new ODVec2(_origin.X + _size.X, _origin.Y);
    public ODVec2 TopLeft => new ODVec2(_origin.X, _origin.Y + _size.Y);
    public ODVec2 TopRight => new ODVec2(_origin.X + _size.X, _origin.Y + _size.Y);

    // Helper method to create from min/max points
    public static ODBoundingBox CreateFromMinMax(ODVec2 min, ODVec2 max)
    {
        return new ODBoundingBox(min, new ODVec2(max.X - min.X, max.Y - min.Y));
    }
}