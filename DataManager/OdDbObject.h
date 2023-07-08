#ifndef OD_DAT_DBOBJECT_H
#define OD_DAT_DBOBJECT_H

/**************************************************************************************
* OpenDraft:    DB Object Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdDbObject.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         July 04, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   DB Object class for OpenDraft framework.                                          *
***************************************************************************************/


#include <fstream>
#include "System/OdCore.h"


using namespace OD::System;


namespace OD
{
	namespace Data
	{
		class OdDrawingDb;

		class OdDbObject {

		// Declare OdDrawingDb as friend class
		friend class OD::Data::OdDrawingDb;

		private:

			unsigned int handle;
			unsigned int parent = 0; // 0 = drawing root

			

		protected:
			void setHandle(unsigned int handle);

		public:
			// Get Type
			virtual const char* getType() const = 0;

			// Getters & Setters
			int getHandle() const;

			int getParent() const;
			void setParent(int parent);

			// Serialise
			//virtual void serialise(std::ostream& aStream) = 0;
			//virtual void deserialise(std::istream& aStream) = 0;

		};

	} // namespace OD::Data
} // namespace OD

#endif // OD_DAT_DBOBJECT_H