using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODMath
{
    public class ODVec2
        {
        public float X { get; set; }
        public float Y { get; set; }
        public ODVec2(float x, float y)
        {
            X = x;
            Y = y;
        }
        public static ODVec2 operator +(ODVec2 a, ODVec2 b)
        {
            return new ODVec2(a.X + b.X, a.Y + b.Y);
        }

        public static ODVec2 operator -(ODVec2 a, ODVec2 b)
        {
            return new ODVec2(a.X - b.X, a.Y - b.Y);
        }

        public static ODVec2 operator *(ODVec2 a, float scalar)
        {
            return new ODVec2(a.X * scalar, a.Y * scalar);
        }
        
        public static ODVec2 operator /(ODVec2 a, float scalar)
        {
            if (scalar == 0)
                throw new DivideByZeroException("Cannot divide by zero.");
            return new ODVec2(a.X / scalar, a.Y / scalar);
        }
        
        public float Magnitude()
        {
            return (float)Math.Sqrt(X * X + Y * Y);
        }
        
        public ODVec2 Normalize()
        {
            float magnitude = Magnitude();
            if (magnitude == 0)
                throw new InvalidOperationException("Cannot normalize a zero vector.");
            return this / magnitude;
        }
        
        public override string ToString()
        {
            return $"({X}, {Y})";
        }
    }
}
