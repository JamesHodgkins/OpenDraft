#ifndef OD_DAT_DBOBJECT_H
#define OD_DAT_DBOBJECT_H

/**************************************************************************************
* OpenDraft:    DB Object Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdDbObject.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         July 04, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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

			unsigned int handle;	// 0 = not set
			unsigned int parent;	// 0 = drawing root

			

		protected:
			void setHandle(unsigned int handle);

		public:

			// Constructors & Destructors
			OdDbObject() : handle(0), parent(-1) {};

			// Get Type
			virtual const char* getType() const = 0;

			// Getters & Setters
			int getHandle() const;

			int getParent() const;
			void setParent(int parent);

			// Serialise
			virtual void serialise(std::wstring &buffer) = 0;
			virtual void deserialise(std::wstring buffer) = 0;

		};

	} // namespace OD::Data
} // namespace OD

#endif // !OD_DAT_DBOBJECT_H