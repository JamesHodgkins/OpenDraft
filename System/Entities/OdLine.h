#ifndef OD_SY_LINE_H
#define OD_SY_LINE_H

/**************************************************************************************
* OpenDraft:    System Line Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdLine.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Line Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/Entities/OdEntity.h"
#include "System/OdSystem.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdLine : public OdEntity {

		private:

			OdVector2 end;


		public:
			
			// Override Get Type
			const char* getType() const
			{
				// Return type
				return "Line";
			}


			//
			// Getters and Setters
			//
			
			OdVector2 getStart() { return location; }
			void setStart(OdVector2 aStart) { location = aStart; }

			OdVector2 getEnd() { return end; }
			void setEnd(OdVector2 aEnd) { location = aEnd; }


			//
			// Serialisation Methods
			//

			virtual void serialise(std::wstring& buffer)
			{
				// For each property, add to buffer

				// Add type code
				buffer += L"E004";

				// Serialise handle
				buffer += std::to_wstring(getHandle());

				// Serialise location
				buffer += std::to_wstring(location.x);
				buffer += std::to_wstring(location.y);

			}

			virtual void deserialise(std::wstring buffer)
			{

			}


			//
			// Constructors
			//
			
			OdLine(OdVector2 aStart, OdVector2 aEnd)
			{
				location = aStart;
				end = aEnd;
			}

			OdLine(int x1, int y1, int x2, int y2)
			{
				location = OdVector2(x1, y1);
				end = OdVector2(x2, y2);
			}


			


			//
			// Calculated Properties
			//

			// Length
			double getLength()
			{
				return sqrt(pow(end.x - location.x, 2) + pow(end.y - location.y, 2));
			}

			// Midpoint
			OdVector2 getMidpoint()
			{
				return OdVector2((location.x + end.x) / 2, (location.y + end.y) / 2);
			}

			// Slope
			double getSlope()
			{
				return (end.y - location.y) / (end.x - location.x);
			}

			// Angle
			double getAngle()
			{
				return atan2(end.y - location.y, end.x - location.x);
			}

			// Perpendicular Slope
			double getPerpendicularSlope()
			{
				return -1 / getSlope();
			}

			// Perpendicular Angle
			double getPerpendicularAngle()
			{
				return atan2(end.x - location.x, location.y - end.y);
			}

			// Perpendicular Bisector
			OdLine getPerpendicularBisector()
			{
				OdVector2 midpoint = getMidpoint();
				double angle = getPerpendicularAngle();
				double length = getLength() / 2;
				OdVector2 location = OdVector2(midpoint.x + length * cos(angle), midpoint.y + length * sin(angle));
				OdVector2 end = OdVector2(midpoint.x - length * cos(angle), midpoint.y - length * sin(angle));
				return OdLine(location, end);
			}

			// Intersection
			OdVector2 getIntersection(OdLine aLine)
			{
				double x1 = location.x;
				double y1 = location.y;
				double x2 = end.x;
				double y2 = end.y;
				double x3 = aLine.location.x;
				double y3 = aLine.location.y;
				double x4 = aLine.end.x;
				double y4 = aLine.end.y;

				double x = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) /
					((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

				double y = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) /
					((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

				return OdVector2(x, y);
			}

			// Closest Point
			OdVector2 closestPointTo(const OdVector2& aPoint) const
			{
				OdVector2 startV = location;
				OdVector2 endV = end;
				OdVector2 point = aPoint;


				OdVector2 line = endV - startV;
				OdVector2 pointToStart = point - startV;
				double t = pointToStart.dot(line) / line.dot(line);

				if (t < 0.0)
					return startV;
				else if (t > 1.0)
					return endV;
				else
					return startV + line * t;
			}


			//
			// Draw
			//
			void draw(NVGcontext* aContext, const OdVector2* aView, const float aScale) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				OdColour drawColour = getDrawColour();
				float lineWeight = getDrawLineWeight();

				float xStart = (location.x + aView->x) * aScale;
				float yStart = (location.y + aView->y) * aScale;
				float xEnd = (end.x + aView->x) * aScale;
				float yEnd = (end.y + aView->y) * aScale;

				// Draw line
				nvgBeginPath(aContext);
				nvgMoveTo(aContext, xStart, yStart);
				nvgLineTo(aContext, xEnd, yEnd);
				nvgStrokeColor(aContext, drawColour.asNvgColour());
				nvgStrokeWidth(aContext, lineWeight);
				nvgStroke(aContext);

				// Draw highlight
				if (isHighlighted())
				{
					nvgBeginPath(aContext);
					nvgMoveTo(aContext, xStart, yStart);
					nvgLineTo(aContext, xEnd, yEnd);
					nvgStrokeColor(aContext, OdColour(255, 255, 255, 100).asNvgColour());
					nvgStrokeWidth(aContext, lineWeight + 2);
					nvgStroke(aContext);
				}
			}

			virtual bool hitTest(OdVector2 aPoint, int aMargin)
			{
				// Is point within margin of line?
				OdVector2 closestPoint = closestPointTo(aPoint);
				double distance = closestPoint.distanceTo(aPoint);
				return distance <= aMargin;
			}

		};
	}
}


#endif // !OD_GEO_LINE_H