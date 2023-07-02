#ifndef OD_SY_ARC_H
#define OD_SY_ARC_H

/**************************************************************************************
* OpenDraft:    System Arc Entity Class                                               *
*-------------------------------------------------------------------------------------*
* Filename:     OdArc.h                                                               *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    �2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Arc Entity class for OpenDraft framework.                                         *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdArc : public OdEntity {
		public:

			OdVector2 center;
			double radius;
			double startAngle;
			double endAngle;


			//
			// Constructors
			//
			OdArc()
			{
				center = OdVector2(0, 0);
				radius = 1;
				startAngle = 0;
				endAngle = 360;
			}

			OdArc(OdVector2 aCenter, double aRadius, double aStartAngle, double aEndAngle)
			{
				center = aCenter;
				radius = aRadius;
				startAngle = aStartAngle;
				endAngle = aEndAngle;
			}

			OdArc(float x, float y, double aRadius, double aStartAngle, double aEndAngle)
			{
				center = OdVector2(x, y);
				radius = aRadius;
				startAngle = aStartAngle;
				endAngle = aEndAngle;
			}

			OdArc(int x, int y, double aRadius, double aStartAngle, double aEndAngle)
			{
				center = OdVector2(x, y);
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
				nvgArc(aContext, center.x, center.y, radius, startAngle, endAngle, NVG_CCW);
				nvgStroke(aContext);
			}


			//
			// Calculates properties
			//

			// Get start point
			OdVector2 startPoint()
			{
				return OdVector2(center.x + radius * cos(startAngle * OD_PI / 180), center.y + radius * sin(startAngle * OD_PI / 180));
			}

			// Get end point
			OdVector2 endPoint()
			{
				return OdVector2(center.x + radius * cos(endAngle * OD_PI / 180), center.y + radius * sin(endAngle * OD_PI / 180));
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