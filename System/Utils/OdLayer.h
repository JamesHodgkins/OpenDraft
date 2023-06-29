#ifndef OD_SY_LAYER_H
#define OD_SY_LAYER_H

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         June 29, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class representing a layer in a drawing.                                        *
***************************************************************************************/


#include <string>


namespace OD
{
	namespace System
	{

		class OdLayer {
		
		private:
			int id;
			int colourIndex;
			int lineWeightIndex;
			int lineTypeIndex;
			std::string name;
			void setId(int aId) { id = aId; }

		public:

			// Constructor
			OdLayer(std::string aName);

			// Getters and setters
			int getId();

			// Colour index
			int getColourIndex();
			void setColourIndex(int aColourIndex);
			
			// Line weight index
			int getLineWeightIndex();
			void setLineWeightIndex(int aLineWeightIndex);
			
			// Line type index
			int getLineTypeIndex();
			void setLineTypeIndex(int aLineTypeIndex);
			
			// Name
			std::string getName();
			void setName(std::string aName);

		};
	}
}


#endif // OD_SY_LAYER_H
