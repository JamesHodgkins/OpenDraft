#ifndef OD_SY_ARC_H
#define OD_SY_ARC_H

/**************************************************************************************
* OpenDraft:    System Arc Entity Class                                               *
*-------------------------------------------------------------------------------------*
* Filename:     OdArc.h                                                               *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Arc Entity class for OpenDraft framework.                                         *
***************************************************************************************/


#include "System/Entities/OdEntity.h"
#include "DataManager/OdData.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdArc : public OdEntity {
		public:

			double radius;
			double startAngle = 90;
			double endAngle = 0;


			// Override Get Type
			const char* getType() const
			{
				// Return type
				return "Arc";
			}

			// Serialisation Methods
			virtual void serialise(std::wstring& buffer)
			{
				// For each property, add to buffer

				// Add type code
				buffer += L"E001";

				// Serialise handle
				buffer += std::to_wstring(getHandle());

				
				buffer += OdData::doubleToHexWString(location.x);
				buffer += OdData::doubleToHexWString(location.y);

				// Serialise radius
				buffer += OdData::doubleToHexWString(radius);

				// Serialise start angle
				buffer += OdData::doubleToHexWString(startAngle);

				// Serialise end angle
				buffer += OdData::doubleToHexWString(endAngle);
			}

			virtual void deserialise(std::wstring buffer)
			{

			}


			//
			// Constructors
			//
			OdArc()
			{
				location = OdVector2(0, 0);
				radius = 1;
				startAngle = 0;
				endAngle = 360;
			}

			OdArc(OdVector2 aCenter, double aRadius, double aStartAngle, double aEndAngle)
			{
				location = aCenter;
				radius = aRadius;
				startAngle = aStartAngle;
				endAngle = aEndAngle;
			}

			OdArc(float x, float y, double aRadius, double aStartAngle, double aEndAngle)
			{
				location = OdVector2(x, y);
				radius = aRadius;
				startAngle = aStartAngle;
				endAngle = aEndAngle;
			}

			OdArc(int x, int y, double aRadius, double aStartAngle, double aEndAngle)
			{
				location = OdVector2(x, y);
				radius = aRadius;
				startAngle = aStartAngle;
				endAngle = aEndAngle;
			}


			//
			// Draw
			//
			void draw(NVGcontext* aContext, const OdVector2* aView, const float aScale) override
			{
				OdColour drawColour = getDrawColour();

				// Calculate scaled centre
				float x = (location.x + aView->x) * aScale;
				float y = (location.y + aView->y) * aScale;

				float a0 = OdMath::deg2rad(startAngle);
				float a1 = OdMath::deg2rad(endAngle);


				// To do: get line weight from layer
				nvgBeginPath(aContext);
				nvgArc(aContext, x, y, radius, a0, a1, NVG_CCW);
				nvgStrokeWidth(aContext, 2.0f);
				nvgStrokeColor(aContext, drawColour.asNvgColour());
				nvgStroke(aContext);

				// Draw highlight
				if (highlight) {
					nvgBeginPath(aContext);
					nvgArc(aContext, x, y, radius, a0, a1, NVG_CCW);
					nvgStrokeWidth(aContext, 4.0f);
					nvgStrokeColor(aContext, OdColour(255,255,255,150).asNvgColour());
					nvgStroke(aContext);
				}
			}

			virtual bool hitTest(OdVector2 aPoint, int aMargin)
			{
				double x = aPoint.x;
				double y = aPoint.y;
				double centerX = location.x;
				double centerY = location.y;
				double rad = radius;
				double a0 = OdMath::deg2rad(startAngle);
				double a1 = OdMath::deg2rad(endAngle);

				// Adjust the arc by adding the margin to the radius
				rad += aMargin;

				// is the point within the radius of the circle?
				bool withinOuter = (aPoint - location).magnitude() <= radius + aMargin;

				// is the point outside the inner radius?
				bool outsideInner = (aPoint - location).magnitude() >= radius - aMargin;


				bool onCircle = (withinOuter && outsideInner);

				if (onCircle)
				{
					// Calculate the angle between the point and the center of the circle
					double angle = atan2(y - centerY, x - centerX);

					// Normalize the angle to be between 0 and 2*pi
					if (angle < 0)
					{
						angle += 2 * OD_PI;
					}

					// Normalize startAngle and endAngle to be between 0 and 2*pi
					if (a0 < 0)
					{
						a0 += 2 * OD_PI;
					}

					if (a1 < 0)
					{
						a1 += 2 * OD_PI;
					}

					// Ensure a0 is less than a1
					if (a0 > a1)
					{
						std::swap(a0, a1);
					}

					// Check if the angle lies outside the arc range
					if (angle < a0 || angle > a1)
					{
						return true; // Point is inside the arc
					}
					
				}

				return false; // Point is outside the arc
				
			}


			//
			// Calculates properties
			//

			// Get start point
			OdVector2 startPoint()
			{
				return OdVector2(location.x + radius * cos(startAngle * OD_PI / 180), location.y + radius * sin(startAngle * OD_PI / 180));
			}

			// Get end point
			OdVector2 endPoint()
			{
				return OdVector2(location.x + radius * cos(endAngle * OD_PI / 180), location.y + radius * sin(endAngle * OD_PI / 180));
			}

			// Get length
			double length()
			{
				return radius * (endAngle - startAngle) * OD_PI / 180;
			}

			// Get area
			double area()
			{
				return 0.5 * radius * radius * (endAngle - startAngle) * OD_PI / 180;
			}

		};
	}
}


#endif // !OD_SY_ARC_H