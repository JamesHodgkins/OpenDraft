using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODGeometry
{
    internal class GeometryTools
    {
        public static bool IsPointOnLine(ODVec2 point, ODVec2 lineStart, ODVec2 lineEnd, double tolerance)
        {
            double distance = DistanceToLineSegment(point, lineStart, lineEnd);
            return distance <= tolerance;
        }

        public static bool IsPointOnCircle(ODVec2 point, ODVec2 circleCenter, double radius, double tolerance)
        {
            double distance = Distance(point, circleCenter);
            // Check if point is within tolerance of the circle's circumference
            return Math.Abs(distance - radius) <= tolerance;
        }

        public static bool IsPointInsideCircle(ODVec2 point, ODVec2 circleCenter, double radius)
        {
            double distance = Distance(point, circleCenter);
            return distance <= radius;
        }

        public static bool IsPointOnRectangle(ODVec2 point, ODRectangle rect, double tolerance)
        {
            // Check if point is near any of the rectangle edges
            ODVec2 topLeft = rect.TopLeft;
            ODVec2 bottomRight = rect.BottomRight;
            ODVec2 topRight = new ODVec2(bottomRight.X, topLeft.Y);
            ODVec2 bottomLeft = new ODVec2(topLeft.X, bottomRight.Y);

            return IsPointOnLine(point, topLeft, topRight, tolerance) ||
                   IsPointOnLine(point, topRight, bottomRight, tolerance) ||
                   IsPointOnLine(point, bottomRight, bottomLeft, tolerance) ||
                   IsPointOnLine(point, bottomLeft, topLeft, tolerance);
        }

        public static bool IsPointOnPolyline(ODVec2 point, List<ODVec2> vertices, double tolerance, bool isClosed = false)
        {
            for (int i = 0; i < vertices.Count - 1; i++)
            {
                if (IsPointOnLine(point, vertices[i], vertices[i + 1], tolerance))
                    return true;
            }

            // Check closing segment for closed polylines
            if (isClosed && vertices.Count > 2)
            {
                if (IsPointOnLine(point, vertices[vertices.Count - 1], vertices[0], tolerance))
                    return true;
            }

            return false;
        }


        public static double DistanceToLineSegment(ODVec2 point, ODVec2 lineStart, ODVec2 lineEnd)
        {
            double lineLengthSquared = (lineEnd.X - lineStart.X) * (lineEnd.X - lineStart.X) +
                                      (lineEnd.Y - lineStart.Y) * (lineEnd.Y - lineStart.Y);

            if (lineLengthSquared == 0)
                return Distance(point, lineStart);

            double t = Math.Max(0, Math.Min(1, ((point.X - lineStart.X) * (lineEnd.X - lineStart.X) +
                                               (point.Y - lineStart.Y) * (lineEnd.Y - lineStart.Y)) / lineLengthSquared));

            ODVec2 projection = new ODVec2(
                lineStart.X + t * (lineEnd.X - lineStart.X),
                lineStart.Y + t * (lineEnd.Y - lineStart.Y)
            );

            return Distance(point, projection);
        }

        public static double Distance(ODVec2 a, ODVec2 b)
        {
            double dx = a.X - b.X;
            double dy = a.Y - b.Y;
            return Math.Sqrt(dx * dx + dy * dy);
        }

        public static ODRectangle CreateBoundingBox(ODVec2 point1, ODVec2 point2)
        {
            return new ODRectangle(
                new ODVec2(Math.Min(point1.X, point2.X), Math.Min(point1.Y, point2.Y)),
                new ODVec2(Math.Max(point1.X, point2.X), Math.Max(point1.Y, point2.Y))
            );
        }

        public static ODRectangle CreateBoundingBox(IEnumerable<ODVec2> points)
        {
            if (!points.Any())
                return new ODRectangle(new ODVec2(0, 0), new ODVec2(0, 0));

            double minX = points.Min(p => p.X);
            double minY = points.Min(p => p.Y);
            double maxX = points.Max(p => p.X);
            double maxY = points.Max(p => p.Y);

            return new ODRectangle(new ODVec2(minX, minY), new ODVec2(maxX, maxY));
        }

        public static ODRectangle InflateBoundingBox(ODRectangle rect, double inflation)
        {
            return new ODRectangle(
                new ODVec2(rect.TopLeft.X - inflation, rect.TopLeft.Y - inflation),
                new ODVec2(rect.BottomRight.X + inflation, rect.BottomRight.Y + inflation)
            );
        }
    }
}
