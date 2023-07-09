#ifndef OD_SY_LINE_H
#define OD_SY_LINE_H

/**************************************************************************************
* OpenDraft:    System Line Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdLine.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Line Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/Entities/OdEntity.h"
#include "System/OdSystem.h"


#define start = location;


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
			// Draw
			//
			void draw(NVGcontext* aContext) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				nvgBeginPath(aContext);
				nvgMoveTo(aContext, start.x, start.y);
				nvgLineTo(aContext, end.x, end.y);
				nvgStrokeColor(aContext, nvgRGBA(255, 0, 0, 255));
				nvgStrokeWidth(aContext, 3);
				nvgStroke(aContext);
			}


			//
			// Calculated Properties
			//

			// Length
			double getLength()
			{
				return sqrt(pow(end.x - start.x, 2) + pow(end.y - start.y, 2));
			}

			// Midpoint
			OdVector2 getMidpoint()
			{
				return OdVector2((start.x + end.x) / 2, (start.y + end.y) / 2);
			}

			// Slope
			double getSlope()
			{
				return (end.y - start.y) / (end.x - start.x);
			}

			// Angle
			double getAngle()
			{
				return atan2(end.y - start.y, end.x - start.x);
			}

			// Perpendicular Slope
			double getPerpendicularSlope()
			{
				return -1 / getSlope();
			}

			// Perpendicular Angle
			double getPerpendicularAngle()
			{
				return atan2(end.x - start.x, start.y - end.y);
			}

			// Perpendicular Bisector
			OdLine getPerpendicularBisector()
			{
				OdVector2 midpoint = getMidpoint();
				double angle = getPerpendicularAngle();
				double length = getLength() / 2;
				OdVector2 start = OdVector2(midpoint.x + length * cos(angle), midpoint.y + length * sin(angle));
				OdVector2 end = OdVector2(midpoint.x - length * cos(angle), midpoint.y - length * sin(angle));
				return OdLine(start, end);
			}

			// Intersection
			OdVector2 getIntersection(OdLine aLine)
			{
				double x1 = start.x;
				double y1 = start.y;
				double x2 = end.x;
				double y2 = end.y;
				double x3 = aLine.start.x;
				double y3 = aLine.start.y;
				double x4 = aLine.end.x;
				double y4 = aLine.end.y;

				double x = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) /
					((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

				double y = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) /
					((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

				return OdVector2(x, y);
			}

		};
	}
}


#endif // OD_SY_LINE_H