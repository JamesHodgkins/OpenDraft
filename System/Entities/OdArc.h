#ifndef OD_SY_ARC_H
#define OD_SY_ARC_H

/**************************************************************************************
* OpenDraft:    System Arc Entity Class                                               *
*-------------------------------------------------------------------------------------*
* Filename:     OdArc.h                                                               *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
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
			double startAngle;
			double endAngle;


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
			void draw(NVGcontext* aContext) override
			{
				// To do: get line weight from layer
				nvgStrokeWidth(aContext, 1.0f);
				nvgStrokeColor(aContext, nvgRGBA(0, 0, 0, 255));
				nvgBeginPath(aContext);
				nvgArc(aContext, location.x, location.y, radius, startAngle, endAngle, NVG_CCW);
				nvgStroke(aContext);
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


#endif // OD_SY_ARC_H